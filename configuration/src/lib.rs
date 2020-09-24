// Dispatcher provides the infrastructure to support the development of DApps,
// mediating the communication between on-chain and off-chain components.

// Copyright (C) 2019 Cartesi Pte. Ltd.

// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.

// This program is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
// PARTICULAR PURPOSE. See the GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// Note: This component currently has dependencies that are licensed under the GNU
// GPL, version 3, and so you should treat this component as a whole as being under
// the GPL version 3. But all Cartesi-written code in this component is licensed
// under the Apache License, version 2, or a compatible permissive license, and can
// be used independently under the Apache v2 license. After this component is
// rewritten, the entire component will be released under the Apache v2 license.

//! Configuration for a cartesi node, including config file, command
//! line arguments and environmental variables.

extern crate env_logger;
extern crate envy;
extern crate error;
extern crate transport;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate structopt;
#[macro_use]
extern crate log;
extern crate db_key;
extern crate ethereum_types;
extern crate hex;
extern crate parity_crypto;
// extern crate rlp;
extern crate serde_json;
extern crate time;
extern crate tokio;
extern crate web3;

const DEFAULT_CONFIG_PATH: &str = "config.yaml";
const DEFAULT_MAX_DELAY: u64 = 500;
const DEFAULT_WARN_DELAY: u64 = 100;

use error::*;
use ethereum_types::{Address, U256};
use parity_crypto::publickey::KeyPair;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use structopt::StructOpt;
use time::Duration;
use transport::GenericTransport;
use web3::futures::Future;

/// A concern is a pair (smart contract, user) that this node should
/// take care of.
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, Copy)]
pub struct Concern {
    pub contract_address: Address,
    pub user_address: Address,
}

impl fmt::Display for Concern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Contract: {},\
             User: {}",
            self.contract_address, self.user_address
        )
    }
}

/// A wrapper for the path of an Ethereum ABI
#[derive(Debug, Clone)]
pub struct ConcernAbi {
    pub abi: PathBuf,
}

/// A worker node, with its ABI and address
#[derive(Debug, Clone)]
pub struct Worker {
    pub abi: PathBuf,
    pub contract_address: Address,
    pub key: ConcernKey,
}

/// A concern together with an ABI
#[derive(Serialize, Deserialize, Debug, Clone)]
struct FullConcern {
    // user_address: Address,
    abi: PathBuf,
}

// impl From<FullConcern> for Concern {
//     fn from(full: FullConcern) -> Self {
//         Concern {
//             contract_address: std::default::Default::default(),
//             user_address: full.user_address,
//         }
//     }
// }

/// Either a key pair, or a single address, to be used either to sign a
/// transaction, or to send an unsigned transaction to an external signer.
#[derive(Clone, Debug)]
pub enum ConcernKey {
    KeyPair(KeyPair),
    UserAddress(Address),
}

impl ConcernKey {
    pub fn address(&self) -> Address {
        match self {
            ConcernKey::KeyPair(key_pair) => key_pair.address(),
            ConcernKey::UserAddress(address) => address.clone(),
        }
    }
}

// In order to use a concern in a key-value disk database, we need to
// implement this Trait.
impl db_key::Key for Concern {
    fn from_u8(key: &[u8]) -> Concern {
        use std::mem::transmute;

        assert!(key.len() == 40);
        let mut result: [u8; 40] = [0; 40];

        for (i, val) in key.iter().enumerate() {
            result[i] = *val;
        }

        unsafe { transmute(result) }
    }

    fn as_slice<T, F: Fn(&[u8]) -> T>(&self, f: F) -> T {
        use std::mem::transmute;

        let val = unsafe { transmute::<_, &[u8; 40]>(self) };
        f(val)
    }
}

impl Concern {
    /// A bytes representation of a concern
    pub fn to_bytes(&self) -> Vec<u8> {
        let result =
            [self.contract_address.as_ref(), self.user_address.as_ref()]
                .concat();
        return Vec::from(&result[..]);
    }
}

/// A transport containing ip address and port
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransPort {
    pub address: String,
    pub port: u16,
}

impl fmt::Display for TransPort {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Address: {},\
             Port: {}",
            self.address, self.port,
        )
    }
}

/// A service containing name and transport
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Service {
    pub name: String,
    pub transport: TransPort,
}

/// Structure for parsing configurations, both Environment and CLI arguments
#[derive(StructOpt, Deserialize, Debug)]
#[structopt(name = "basic")]
struct EnvCLIConfiguration {
    /// Path to configuration file
    #[structopt(short = "c", long = "config_path")]
    config_path: Option<String>,
    /// Url for the Ethereum node
    #[structopt(short = "u", long = "url")]
    url: Option<String>,
    /// Indicates the use of a testing environment
    #[structopt(short = "t", long = "testing")]
    testing: Option<bool>,
    /// Indicates the maximal possible delay acceptable for the Ethereum node
    #[structopt(short = "m", long = "maximum")]
    max_delay: Option<u64>,
    /// Level of delay for Ethereum node that should trigger warnings
    #[structopt(short = "w", long = "warn")]
    warn_delay: Option<u64>,
    /// Main concern's user address
    #[structopt(long = "concern_user")]
    main_concern_user: Option<String>,
    /// Main concern's contract's abi
    #[structopt(long = "concern_abi")]
    main_concern_abi: Option<String>,
    /// Working path
    #[structopt(long = "working_path")]
    working_path: Option<String>,
    /// Port for emulator grpc
    #[structopt(long = "emulator_port")]
    emulator_port: Option<u16>,
    /// Address for emulator grpc
    #[structopt(long = "emulator_address")]
    emulator_address: Option<String>,
    /// Port used to make queries
    #[structopt(long = "query_port")]
    query_port: Option<u16>,
    /// Number of confirmations for transaction
    #[structopt(long = "confirmations")]
    confirmations: Option<usize>,
    /// Interval of polling the blockchain (in seconds)
    #[structopt(long = "polling_interval")]
    polling_interval: Option<u64>,
    #[structopt(long = "web3_timeout")]
    web3_timeout: Option<u64>,
    /// Main concern's contract's abi
    #[structopt(long = "worker_abi")]
    worker_abi: Option<String>,
}

/// Structure to parse configuration from file
#[derive(Serialize, Deserialize, Debug, Clone)]
struct FileConfiguration {
    url: Option<String>,
    testing: Option<bool>,
    max_delay: Option<u64>,
    warn_delay: Option<u64>,
    main_concern: Option<FullConcern>,
    user_address: Option<String>,
    contracts: Option<HashMap<String, FullConcern>>,
    concerns: Vec<FullConcern>,
    working_path: Option<String>,
    services: Vec<Service>,
    query_port: Option<u16>,
    confirmations: Option<usize>,
    polling_interval: Option<u64>,
    web3_timeout: Option<u64>,
    worker_abi: Option<String>,
}

/// Configuration after parsing
#[derive(Debug, Clone)]
pub struct Configuration {
    pub url: String,
    pub testing: bool,
    pub max_delay: Duration,
    pub warn_delay: Duration,
    pub main_concern: Concern,
    pub contracts: HashMap<String, Concern>,
    pub concerns: Vec<Concern>,
    pub working_path: PathBuf,
    pub abis: HashMap<Concern, ConcernAbi>,
    pub services: Vec<Service>,
    pub query_port: u16,
    pub confirmations: usize,
    pub polling_interval: u64,
    pub web3_timeout: u64,
    pub chain_id: u64,
    pub signer_key: ConcernKey,
    pub worker: Option<Worker>,
}

/// check if a given transport is well formed (having all valid arguments).
fn validate_transport(
    validate_address: String,
    validate_port: u16,
) -> Result<TransPort> {
    Ok(TransPort {
        address: validate_address,
        port: validate_port,
    })
}

// !!!!!!!!!!!
// update this
// !!!!!!!!!!!
impl fmt::Display for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{ Url: {}, \
             Testing: {}, \
             Max delay: {}, \
             Warning delay: {}, \
             Main concern: {}, \
             Number of concerns: {}, \
             Working path: {:?}, \
             Number of services: {}, \
             Number of confirmations: {}, \
             Query port: {}, \
             Using external signer: {:?}",
            self.url,
            self.testing,
            self.max_delay,
            self.warn_delay,
            self.main_concern,
            self.concerns.len(),
            self.working_path,
            self.services.len(),
            self.confirmations,
            self.query_port,
            self.signer_key
        )
    }
}

impl Configuration {
    /// Creates a Configuration from a file as well as the Environment
    /// and CLI arguments
    pub fn new() -> Result<Configuration> {
        info!("Load config from CLI arguments");
        let cli_config = EnvCLIConfiguration::from_args();
        info!("CLI args: {:?}", cli_config); // implement Display instead

        info!("Load config from environment variables");
        let env_config =
            envy::prefixed("CARTESI_").from_env::<EnvCLIConfiguration>()?;
        info!("Env args: {:?}", env_config); // implement Display instead

        info!("Load config from file");
        let config_path = cli_config
            .config_path
            .as_ref()
            .or(env_config.config_path.as_ref())
            .unwrap_or(&DEFAULT_CONFIG_PATH.to_string())
            .clone();
        info!("Config file path is {}", config_path.clone());
        let mut file = File::open(&config_path[..]).chain_err(|| {
            format!("unable to read configuration file: {}", config_path)
        })?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).chain_err(|| {
            format!("could not read from configuration file: {}", config_path)
        })?;
        info!("File args: {}", contents); // implement Display instead

        let file_config: FileConfiguration =
            serde_yaml::from_str(&contents[..])
                .map_err(|e| error::Error::from(e))
                .chain_err(|| {
                    format!(
                        "could not parse configuration file: {}",
                        config_path
                    )
                })?;
        info!("File config: {:?}", file_config.clone());

        // merge these three configurations
        let config = combine_config(cli_config, env_config, file_config)?;
        info!("Combined args: {}", config);

        // check if max_delay and warn delay are compatible
        if config.max_delay < config.warn_delay {
            return Err(Error::from(ErrorKind::InvalidConfig(
                "max_delay should be larger than warn delay".into(),
            )));
        }

        Ok(config)
    }
}

fn parse_abi(abi: Option<String>) -> Result<PathBuf> {
    abi.ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
        "Concern's abi should be specified",
    ))))?
    .parse()
    .chain_err(|| format!("failed to parse contract's abi"))
}

fn parse_user_address(user: Option<String>) -> Result<Address> {
    user.ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
        "Concern's user should be specified",
    ))))?
    .trim_start_matches("0x")
    .parse()
    .chain_err(|| format!("failed to parse user address"))
}

/// Combines the three configurations from: CLI, Environment and file.
fn combine_config(
    cli_config: EnvCLIConfiguration,
    env_config: EnvCLIConfiguration,
    file_config: FileConfiguration,
) -> Result<Configuration> {
    // determine url (cli -> env -> config)
    let url: String = cli_config
        .url
        .or(env_config.url)
        .or(file_config.url)
        .ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
            "Need to provide url (config file, command line or env)",
        ))))?;

    // determine web3 timeout (cli -> env -> config)
    let web3_timeout: u64 = cli_config
        .web3_timeout
        .or(env_config.web3_timeout)
        .or(file_config.web3_timeout)
        .unwrap_or(10);

    info!("Trying to connect to Eth node at {}", &url[..]);
    let (_eloop, transport) = GenericTransport::new(&url[..], web3_timeout)
        .chain_err(|| {
            format!("could not connect to Eth node at url: {}", &url)
        })?;

    info!("Testing Ethereum node's functionality");
    let url_clone = url.clone();
    let web3 = web3::Web3::new(transport);
    let network_id: String = web3
        .net()
        .version()
        .map_err(move |e| {
            error!("{}", e);
            Error::from(ErrorKind::ChainError(format!(
                "no Ethereum node responding at url: {}",
                url_clone
            )))
        })
        .wait()?;
    info!("Connected to Ethereum node with network id {}", &network_id);

    let url_clone = url.clone();
    let chain_id: u64 = web3
        .eth()
        .chain_id()
        .map_err(move |e| {
            error!("{}", e);
            Error::from(ErrorKind::ChainError(format!(
                "no Ethereum node responding at url: {}",
                url_clone
            )))
        })
        .wait()?
        .as_u64();

    // determine if using external signer, by checking if there's no
    // concern key.
    let signer_key = if std::env::var("CARTESI_CONCERN_KEY").is_err() {
        let accounts = web3
            .eth()
            .accounts()
            .map_err(move |e| {
                error!("{}", e);
                Error::from(ErrorKind::ChainError(format!(
                    "Could not get user_address from external signer",
                )))
            })
            .wait()?;
        if !accounts.is_empty() {
            ConcernKey::UserAddress(accounts[0])
        } else {
            return Err(Error::from(ErrorKind::ChainError(format!(
                "External signer returned zero accounts",
            ))));
        }
    } else {
        let key =
            recover_key().chain_err(|| "could not find key for concern")?;
        ConcernKey::KeyPair(key)
    };

    // determine testing (cli -> env -> config)
    let testing: bool = cli_config
        .testing
        .or(env_config.testing)
        .or(file_config.testing)
        .unwrap_or(false);

    // determine max_delay (cli -> env -> config)
    let max_delay = cli_config
        .max_delay
        .or(env_config.max_delay)
        .or(file_config.max_delay)
        .unwrap_or(DEFAULT_MAX_DELAY);

    // determine warn_delay (cli -> env -> config)
    let warn_delay = cli_config
        .warn_delay
        .or(env_config.warn_delay)
        .or(file_config.warn_delay)
        .unwrap_or(DEFAULT_WARN_DELAY);

    // determine working path (cli -> env -> config)
    let working_path = PathBuf::from(&cli_config
        .working_path
        .or(env_config.working_path)
        .or(file_config.working_path)
        .ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
            "Need to provide working path (config file, command line or env)",
        ))))?);

    info!("determine worker abi");
    let worker = {
        let abi = cli_config
            .worker_abi
            .or(env_config.worker_abi)
            .or(file_config.worker_abi)
            .and_then(|path| Some(PathBuf::from(&path)));

        match abi {
            Some(abi) => {
                let address =
                    get_contract_address(abi.clone(), network_id.clone())?;
                Some(Worker {
                    abi: abi,
                    contract_address: address,
                    key: signer_key.clone(),
                })
            }
            None => None,
        }
    };

    info!("determine user address");
    let user_address = {
        let config_address = cli_config
            .main_concern_user
            .or(env_config.main_concern_user)
            .or(file_config.user_address);

        match (config_address, &worker) {
            (Some(address), _) => parse_user_address(Some(address))?,
            (None, Some(worker)) => accept_job(&web3, worker)?,
            (None, None) => {
                return Err(Error::from(ErrorKind::InvalidConfig(
                    String::from(
                        "Need either a user_addess or a worker defined",
                    ),
                )));
            }
        }
    };

    // determine no two services has the same name,
    // and the transports are valid
    info!("validate services transport and names");
    let mut name_set = HashSet::new();
    for service in &file_config.services {
        if !name_set.insert(service.name.clone()) {
            return Err(Error::from(ErrorKind::InvalidConfig(format!(
                "Duplicate service names found: {}",
                service.name
            ))));
        }
        let _transport = validate_transport(
            service.transport.address.clone(),
            service.transport.port,
        )?;
    }

    let query_port: u16 = cli_config
        .query_port
        .or(env_config.query_port)
        .or(file_config.query_port)
        .ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
            "Need a port for queries (config file, command line or env)",
        ))))?;

    // determine number of confirmations (cli -> env -> config)
    let confirmations: usize = cli_config
        .confirmations
        .or(env_config.confirmations)
        .or(file_config.confirmations)
        .ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
            "Need a number of confirmations (config file, command line or env)",
        ))))?;

    // determine polling interval (cli -> env -> config)
    let polling_interval: u64 = cli_config
        .polling_interval
        .or(env_config.polling_interval)
        .or(file_config.polling_interval)
        .unwrap_or(6);

    info!("build main concern");
    let main_concern =
        cli_config.main_concern_abi.or(env_config.main_concern_abi);

    let main_concern = match (main_concern, file_config.main_concern) {
        (Some(s), _) => parse_abi(Some(s))?,
        (None, Some(c)) => c.abi,
        (None, None) => {
            return Err(Error::from(ErrorKind::InvalidConfig(String::from(
                "Need to provide main concern (config file, command line or env)",
            ))));
        }
    };

    let full_concerns = file_config.concerns;

    let mut abis: HashMap<Concern, ConcernAbi> = HashMap::new();
    let mut concerns: Vec<Concern> = vec![];

    // insert all full concerns into concerns and abis
    for full_concern in full_concerns {
        info!("Insert full concern {:?}", full_concern);
        let contract_address =
            get_contract_address(full_concern.abi.clone(), network_id.clone())?;

        let concern: Concern = Concern {
            contract_address: contract_address,
            user_address: user_address,
        };

        // store concern data in hash table
        abis.insert(
            concern.clone(),
            ConcernAbi {
                abi: full_concern.abi.clone(),
            },
        );
        concerns.push(concern);
    }

    let mut contracts: HashMap<String, Concern> = HashMap::new();
    if let Some(contract_full_concerns) = file_config.contracts {
        // insert all contract concerns into concerns and abis
        for (name, full_concern) in contract_full_concerns.iter() {
            info!("Insert contract {:?}, {:?}", name, full_concern);
            let contract_address = get_contract_address(
                full_concern.abi.clone(),
                network_id.clone(),
            )?;

            let concern: Concern = Concern {
                contract_address: contract_address,
                user_address: user_address,
            };

            // store concern data in hash table
            abis.insert(
                concern.clone(),
                ConcernAbi {
                    abi: full_concern.abi.clone(),
                },
            );
            contracts.insert(name.clone(), concern.clone());
            concerns.push(concern);
        }
    }

    info!("Get main concern address: {:?}", main_concern);
    let contract_address =
        get_contract_address(main_concern.clone(), network_id.clone())?;

    let concern: Concern = Concern {
        contract_address: contract_address,
        user_address: user_address,
    };

    // insert main full concern in concerns and abis
    abis.insert(
        concern.clone(),
        ConcernAbi {
            abi: main_concern.clone(),
        },
    );
    concerns.push(concern.clone());

    Ok(Configuration {
        url: url,
        testing: testing,
        max_delay: Duration::seconds(max_delay as i64),
        warn_delay: Duration::seconds(warn_delay as i64),
        main_concern: concern,
        contracts: contracts,
        concerns: concerns,
        working_path: working_path,
        abis: abis,
        services: file_config.services,
        query_port: query_port,
        confirmations: confirmations,
        polling_interval: polling_interval,
        web3_timeout: web3_timeout,
        chain_id: chain_id,
        signer_key: signer_key,
        worker: worker,
    })
}

fn get_contract_address(abi: PathBuf, network_id: String) -> Result<Address> {
    let mut file = File::open(&abi)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    let v: Value = serde_json::from_str(&s[..])
        .chain_err(|| format!("could not read contract json file"))?;

    // retrieve the contract address (supports both truffle and buidler formats)
    let contract_address_option = v["networks"][&network_id]["address"]
        .as_str()
        .or(v["address"].as_str());
    let contract_address_str = match contract_address_option {
        Some(address_str) => address_str.split_at(2).1,
        None => {
            return Err(Error::from(ErrorKind::InvalidConfig(format!(
            "Fail to parse contract address from file {} with network id {}",
            abi.display(),
            &network_id
        ))))
        }
    };
    let contract_address: Address = contract_address_str.parse()?;

    Ok(contract_address)
}

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// we need to implement recovering keys in keystore
// the current method uses environmental variables
// and it is not safe enough
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
fn recover_key() -> Result<KeyPair> {
    info!("Recovering key from environment variable");
    let key_string: String =
        std::env::var("CARTESI_CONCERN_KEY").chain_err(|| {
            format!(
                "for now, keys must be provided as env variable, provide one"
            )
        })?;
    let key_pair = KeyPair::from_secret(
        key_string
            .trim_start_matches("0x")
            .parse()
            .chain_err(|| format!("failed to parse key"))?,
    )?;
    Ok(key_pair)
}

// Worker accept job if needed
fn accept_job(
    web3: &web3::Web3<GenericTransport>,
    worker: &Worker,
) -> Result<Address> {
    info!("Start accept job");
    // Load abi
    let mut file = File::open(worker.abi.clone())?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    let v: Value = serde_json::from_str(&s[..])
        .chain_err(|| format!("could not read truffle json file"))?;

    // create a contract object
    let contract = web3::contract::Contract::from_json(
        web3.eth().clone(),
        worker.contract_address,
        serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
    )
    .chain_err(|| format!("could not create contract for worker"))?;

    // Create a low level abi for worker contract
    let abi = ethabi::Contract::load(
        serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
    )
    .chain_err(|| format!("Could not create low level abi for worker"))?;

    loop {
        info!("Getting worker state");
        let worker_state = get_worker_state(&contract, worker)?;
        info!("Worker state: {:?}", worker_state);

        match worker_state {
            WorkerState::Available => (),
            WorkerState::Pending(_) => {
                // Accept job
                send_accept_job(web3, &abi, worker)?;
            }
            WorkerState::Owned(owner_address) => {
                return Ok(owner_address);
            }
            WorkerState::Retired(_) => {
                // If worker is retired, stop and return error. This error is unrecoverable.
                return Err(Error::from(format!("Worker is retired")));
            }
        }

        // Wait 15 seconds before trying again.
        std::thread::sleep(std::time::Duration::from_secs(15));
    }
}

#[derive(Debug, Clone)]
enum WorkerState {
    Available,
    Pending(Address),
    Owned(Address),
    Retired(Address),
}

fn get_worker_state(
    contract: &web3::contract::Contract<GenericTransport>,
    worker: &Worker,
) -> Result<WorkerState> {
    let (query_result, user_address): (_, Address) =
        web3::futures::future::join_all(vec![
            build_worker_state_query("isAvailable", contract, worker),
            build_worker_state_query("isPending", contract, worker),
            build_worker_state_query("isOwned", contract, worker),
            build_worker_state_query("isRetired", contract, worker),
        ])
        .join(contract.query(
            "getUser",
            worker.key.address(),
            None,
            web3::contract::Options::default(),
            None,
        ))
        .wait()?;

    let query_result = (
        query_result[0],
        query_result[1],
        query_result[2],
        query_result[3],
    );

    match query_result {
        (true, _, _, _) => Ok(WorkerState::Available),
        (_, true, _, _) => Ok(WorkerState::Pending(user_address)),
        (_, _, true, _) => Ok(WorkerState::Owned(user_address)),
        (_, _, _, true) => Ok(WorkerState::Retired(user_address)),
        (false, false, false, false) => {
            Err(Error::from(format!("Invalid blockchain state")))
        }
    }
}

fn build_worker_state_query(
    func: &str,
    contract: &web3::contract::Contract<GenericTransport>,
    worker: &Worker,
) -> web3::contract::QueryResult<
    bool,
    std::boxed::Box<
        dyn tokio::prelude::Future<
                Item = serde_json::Value,
                Error = web3::Error,
            > + std::marker::Send,
    >,
> {
    contract.query(
        func,
        worker.key.address(),
        None,
        web3::contract::Options::default(),
        None,
    )
}

fn send_accept_job(
    web3: &web3::Web3<GenericTransport>,
    abi: &ethabi::Contract,
    worker: &Worker,
) -> Result<()> {
    trace!("Accept job transaction...");
    let gas_price = web3.eth().gas_price().wait()?;
    let gas_price = U256::from(2).checked_mul(gas_price).unwrap();

    abi.function("acceptJob")
        .and_then(|function| function.encode_input(&vec![]))
        .map(move |data| {
            match &worker.key {
                ConcernKey::UserAddress(address) => {
                    let tx_request = web3::types::TransactionRequest {
                        from: *address,
                        to: Some(worker.contract_address),
                        gas_price: Some(gas_price),
                        gas: None,
                        value: None,
                        data: Some(web3::types::Bytes(data)),
                        condition: None,
                        nonce: None,
                    };

                    trace!("Sending unsigned transaction");
                    web3.eth()
                        .send_transaction(tx_request)
                        .map(|hash| {
                            info!("Transaction sent with hash: {:?}", hash);
                        })
                        .or_else(|e| {
                            // ignore the nonce error, by pass the other errors
                            if let web3::error::Error::Rpc(ref rpc_error) = e {
                                let nonce_error = String::from(
                                    "the tx doesn't have the correct nonce",
                                );
                                if rpc_error.message[..nonce_error.len()]
                                    == nonce_error
                                {
                                    warn!(
                                        "Ignoring nonce Error: {}",
                                        rpc_error.message
                                    );
                                    return Box::new(
                                        web3::futures::future::ok::<(), _>(()),
                                    );
                                }
                            }
                            return Box::new(web3::futures::future::err(e));
                        })
                        .map_err(|e| {
                            warn!("Failed to send transaction. Error {}", e);
                            error::Error::from(e)
                        })
                        .wait()?;

                    Ok(())
                }
                ConcernKey::KeyPair(_) => Err(Error::from(format!(
                    "Local signing not suported for worker"
                ))),
            }
        })?
}
