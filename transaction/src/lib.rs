extern crate configuration;
extern crate env_logger;
extern crate envy;
extern crate error;

#[macro_use]
extern crate serde_derive;
extern crate structopt;
#[macro_use]
extern crate log;
extern crate ethcore_transaction;
extern crate ethereum_types;
extern crate ethjson;
extern crate ethkey;
extern crate keccak_hash;

use configuration::Configuration;
use error::*;
use ethcore_transaction::{Action, SignedTransaction, Transaction};
use ethereum_types::{Address, U256};
use ethkey::{KeyPair, Public, Secret};
use keccak_hash::keccak;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use structopt::StructOpt;

pub struct TransactionManager {
    config: Configuration,
    keys: HashMap<Address, Secret>,
    secret: Secret,
}

const ADDR: &str = "0x2ad38f50f38abc5cbcf175e1962293eecc7936de";

const KEY: &str =
    "339565dd96968ad4fba67e320bc9cf07808298d3654634e1bcc3b46350964f6e";

impl TransactionManager {
    pub fn new(config: Configuration) -> TransactionManager {
        // Change this by a properly handled ethstore
        let mut hard_coded_keys = HashMap::new();

        hard_coded_keys.insert(Address::from(ADDR), Secret::from(KEY));

        TransactionManager {
            config: config,
            keys: hard_coded_keys,
            secret: Secret::from(KEY),
        }
    }

    pub fn send(&self) {
        let key = KeyPair::from_secret(self.secret.clone()).unwrap();
        let t = Transaction {
            action: Action::Call(key.address()),
            nonce: U256::from(42),
            gas_price: U256::from(3000),
            gas: U256::from(50_000),
            value: U256::from(1),
            data: b"Hello!".to_vec(),
        }
        .sign(&key.secret(), Some(69));
        assert_eq!(Address::from(keccak(key.public())), t.sender());
        assert_eq!(t.chain_id(), Some(69));
    }
}

// /// A key pair
// #[derive(Serialize, Deserialize, Debug)]
// pub struct KeyPair {
//     contract: String,
//     user: String,
// }

// /// Structure for parsing configurations, both Environment and CLI arguments
// #[derive(StructOpt, Deserialize, Debug)]
// #[structopt(name = "basic")]
// struct EnvCLIConfiguration {
//     /// Url for the Ethereum node
//     #[structopt(short = "u", long = "url")]
//     url: Option<String>,
//     /// Indicates the use of a testing environment
//     #[structopt(short = "t", long = "testing")]
//     testing: Option<bool>,
//     /// Indicates the maximal possible delay acceptable for the Ethereum node
//     #[structopt(short = "m", long = "maximum")]
//     max_delay: Option<u64>,
//     /// Level of delay for Ethereum node that should trigger warnings
//     #[structopt(short = "w", long = "warn")]
//     warn_delay: Option<u64>,
//     /// Special concern's contract
//     #[structopt(long = "concern_contract")]
//     concern_contract: Option<String>,
//     /// Special concern's user address
//     #[structopt(long = "concern_user")]
//     concern_user: Option<String>,
// }

// /// Structure to parse configuration from file
// #[derive(Serialize, Deserialize, Debug)]
// struct FileConfiguration {
//     url: Option<String>,
//     testing: Option<bool>,
//     max_delay: Option<u64>,
//     warn_delay: Option<u64>,
//     concerns: Vec<Concern>,
// }

// /// Configuration after parsing
// #[derive(Debug)]
// pub struct Configuration {
//     pub url: String,
//     pub testing: bool,
//     pub max_delay: u64,
//     pub warn_delay: u64,
//     pub concerns: Vec<Concern>,
// }

// impl Configuration {
//     /// Creates a Configuration from a file as well as the Environment
//     /// and CLI arguments
//     pub fn new(path: &str) -> Result<Configuration> {
//         // try to load config from CLI arguments
//         let cli_config = EnvCLIConfiguration::from_args();
//         info!("CLI args: {:?}", cli_config);
//         println!("Bla");

//         // try to load config from env
//         let env_config = envy::from_env::<EnvCLIConfiguration>()?;
//         info!("Env args: {:?}", env_config);

//         // try to read config from file
//         let mut file = File::open(path).chain_err(|| {
//             format!("unable to read configuration file: {}", path)
//         })?;
//         let mut contents = String::new();
//         file.read_to_string(&mut contents).chain_err(|| {
//             format!("could not read from configuration file: {}", path)
//         })?;
//         let file_config: FileConfiguration =
//             serde_yaml::from_str(&contents[..])
//                 .map_err(|e| error::Error::from(e))
//                 .chain_err(|| {
//                     format!("could not parse configuration file: {}", path)
//                 })?;

//         // merge these three configurations
//         let config = combine_config(cli_config, env_config, file_config)?;
//         info!("Combined args: {:?}", config);

//         // check if max_delay and warn delay are compatible
//         if config.max_delay < config.warn_delay {
//             return Err(Error::from(ErrorKind::InvalidConfig(
//                 "max_delay should be larger than warn delay".into(),
//             )));
//         }

//         Ok(config)
//     }
// }

// impl fmt::Display for Configuration {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(
//             f,
//             "{{ Url: {}\
//              Testing: {}\
//              Max delay: {}\
//              Warning delay: {}\
//              Number of concerns: {} }}",
//             self.url,
//             self.testing,
//             self.max_delay,
//             self.warn_delay,
//             self.concerns.len()
//         )
//     }
// }

// /// check if a given concern is well formed (either having all arguments
// /// or none).
// fn validate_concern(
//     contract: Option<String>,
//     user: Option<String>,
// ) -> Result<Option<Concern>> {
//     // if some option is Some, both should be
//     if contract.is_some() || user.is_some() {
//         Ok(Some(Concern {
//             contract: contract.ok_or(Error::from(ErrorKind::InvalidConfig(
//                 String::from("All fields of concern should be specified"),
//             )))?,
//             user: user.ok_or(Error::from(ErrorKind::InvalidConfig(
//                 String::from("All fields of concern should be specified"),
//             )))?,
//         }))
//     } else {
//         Ok(None)
//     }
// }

// /// Combines the three configurations from: CLI, Environment and file.
// fn combine_config(
//     cli_config: EnvCLIConfiguration,
//     env_config: EnvCLIConfiguration,
//     file_config: FileConfiguration,
// ) -> Result<Configuration> {
//     // determine url (cli -> env -> config)
//     let url: String = cli_config
//         .url
//         .or(env_config.url)
//         .or(file_config.url)
//         .ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
//             "Need to provide url (config file, command line or env)",
//         ))))?;

//     // determine testing (cli -> env -> config)
//     let testing: bool = cli_config
//         .testing
//         .or(env_config.testing)
//         .or(file_config.testing)
//         .unwrap_or(false);

//     // determine max_delay (cli -> env -> config)
//     let max_delay = cli_config
//         .max_delay
//         .or(env_config.max_delay)
//         .or(file_config.max_delay)
//         .unwrap_or(DEFAULT_MAX_DELAY);

//     // determine warn_delay (cli -> env -> config)
//     let warn_delay = cli_config
//         .warn_delay
//         .or(env_config.warn_delay)
//         .or(file_config.warn_delay)
//         .unwrap_or(DEFAULT_WARN_DELAY);

//     info!("determine cli concern");
//     let cli_concern =
//         validate_concern(cli_config.concern_contract, cli_config.concern_user)?;

//     info!("determine env concern");
//     let env_concern =
//         validate_concern(env_config.concern_contract, env_config.concern_user)?;

//     // determine concerns (start from file and push CLI and Environment)
//     let mut concerns = file_config.concerns;
//     if let Some(c) = cli_concern {
//         concerns.push(c);
//     };
//     if let Some(c) = env_concern {
//         concerns.push(c);
//     };

//     Ok(Configuration {
//         url: url,
//         testing: testing,
//         max_delay: max_delay,
//         warn_delay: warn_delay,
//         concerns: concerns,
//     })
// }
