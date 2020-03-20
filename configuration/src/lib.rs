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
extern crate time;
extern crate web3;
extern crate serde_json;

const DEFAULT_CONFIG_PATH: &str = "config.yaml";
const DEFAULT_MAX_DELAY: u64 = 500;
const DEFAULT_WARN_DELAY: u64 = 100;

use error::*;
use ethereum_types::Address;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use structopt::StructOpt;
use time::Duration;
use web3::futures::Future;
use serde_json::Value;
use transport::GenericTransport;

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

/// A concern together with an ABI
#[derive(Serialize, Deserialize, Debug, Clone)]
struct FullConcern {
    user_address: Address,
    abi: PathBuf,
}

impl From<FullConcern> for Concern {
    fn from(full: FullConcern) -> Self {
        Concern {
            contract_address: std::default::Default::default(),
            user_address: full.user_address,
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
        let mut result: [u8; 40] = [0; 40];
        self.contract_address.copy_to(&mut result[0..20]);
        self.user_address.copy_to(&mut result[20..40]);
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
            self.address, self.port
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
    web3_timeout: Option<u64>
}

/// Structure to parse configuration from file
#[derive(Serialize, Deserialize, Debug, Clone)]
struct FileConfiguration {
    url: Option<String>,
    testing: Option<bool>,
    max_delay: Option<u64>,
    warn_delay: Option<u64>,
    main_concern: Option<FullConcern>,
    concerns: Vec<FullConcern>,
    working_path: Option<String>,
    services: Vec<Service>,
    query_port: Option<u16>,
    confirmations: Option<usize>,
    polling_interval: Option<u64>,
    web3_timeout: Option<u64>
}

/// Configuration after parsing
#[derive(Debug, Clone)]
pub struct Configuration {
    pub url: String,
    pub testing: bool,
    pub max_delay: Duration,
    pub warn_delay: Duration,
    pub main_concern: Concern,
    pub concerns: Vec<Concern>,
    pub working_path: PathBuf,
    pub abis: HashMap<Concern, ConcernAbi>,
    pub services: Vec<Service>,
    pub query_port: u16,
    pub confirmations: usize,
    pub polling_interval: u64,
    pub web3_timeout: u64,
    pub chain_id: u64
}

/// check if a given transport is well formed (having all valid arguments).
fn validate_transport(
    validate_address: String,
    validate_port: u16,
) -> Result<TransPort> {
    Ok(TransPort {
        address: validate_address,
        port: validate_port
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
             Query port: {}",
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

/// check if a given concern is well formed (either having all arguments
/// or none).
fn validate_concern(
    user: Option<String>,
    abi: Option<String>,
) -> Result<Option<FullConcern>> {
    // if some option is Some, both should be
    if user.is_some() || abi.is_some() {
        Ok(Some(FullConcern {
            user_address: user
                .ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
                    "Concern's user should be specified",
                ))))?
                .trim_start_matches("0x")
                .parse()
                .chain_err(|| format!("failed to parse user address"))?,
            abi: abi
                .ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
                    "Concern's abi should be specified",
                ))))?
                .parse()
                .chain_err(|| format!("failed to parse contract's abi"))?,
        }))
    } else {
        Ok(None)
    }
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

    // determine no two services has the same name,
    // and the transports are valid
    info!("validate services transport and names");
    let mut name_set = HashSet::new();
    for service in &file_config.services {
        if !name_set.insert(service.name.clone()) {
            return Err(Error::from(ErrorKind::InvalidConfig(format!(
                "Duplicate service names found: {}", service.name
            ))));
        }
        let _transport = validate_transport(service.transport.address.clone(), service.transport.port)?;
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

    info!("determine cli concern");
    let cli_main_concern = validate_concern(
        cli_config.main_concern_user,
        cli_config.main_concern_abi,
    )?;

    info!("determine env concern");
    let env_main_concern = validate_concern(
        env_config.main_concern_user,
        env_config.main_concern_abi,
    )?;

    let full_concerns = file_config.concerns;

    // determine main concern (cli -> env -> config)
    let main_concern = cli_main_concern
        .or(env_main_concern)
        .or(file_config.main_concern)
        .ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
            "Need to provide main concern (config file, command line or env)",
        ))))?;

    let mut abis: HashMap<Concern, ConcernAbi> = HashMap::new();
    let mut concerns: Vec<Concern> = vec![];

    // insert all full concerns into concerns and abis
    for full_concern in full_concerns {
        let contract_address = get_contract_address(full_concern.abi.clone(), network_id.clone())?;

        let mut concern: Concern = full_concern.clone().into();
        concern.contract_address = contract_address;

        // store concern data in hash table
        abis.insert(
            concern.clone(),
            ConcernAbi {
                abi: full_concern.abi.clone(),
            },
        );
        concerns.push(concern);
    }

    let contract_address = get_contract_address(main_concern.abi.clone(), network_id.clone())?;

    let mut concern: Concern = main_concern.clone().into();
    concern.contract_address = contract_address;

    // insert main full concern in concerns and abis
    abis.insert(
        concern.clone(),
        ConcernAbi {
            abi: main_concern.abi.clone(),
        },
    );
    concerns.push(concern.clone());

    Ok(Configuration {
        url: url,
        testing: testing,
        max_delay: Duration::seconds(max_delay as i64),
        warn_delay: Duration::seconds(warn_delay as i64),
        main_concern: concern,
        concerns: concerns,
        working_path: working_path,
        abis: abis,
        services: file_config.services,
        query_port: query_port,
        confirmations: confirmations,
        polling_interval: polling_interval,
        web3_timeout: web3_timeout,
        chain_id: chain_id,
    })
}

fn get_contract_address(abi: PathBuf, network_id: String) -> Result<Address> {
    let mut file = File::open(&abi)?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    let v: Value = serde_json::from_str(&s[..])
        .chain_err(|| format!("could not read truffle json file"))?;

    // retrieve the contract address
    let contract_address_str = match v["networks"][&network_id]["address"].as_str() {
        Some(address_str) => address_str.split_at(2).1,
        None => return Err(Error::from(ErrorKind::InvalidConfig(format!(
            "Fail to parse contract address from file {} with network id {}", abi.display(), &network_id
        ))))
    };
    let contract_address: Address = contract_address_str.parse()?;

    Ok(contract_address)
}