extern crate env_logger;
extern crate envy;
extern crate error;

#[macro_use]
extern crate serde_derive;
extern crate structopt;
#[macro_use]
extern crate log;

//extern crate ethabi;

const DEFAULT_MAX_DELAY: u64 = 500;
const DEFAULT_WARN_DELAY: u64 = 100;

use error::*;
use std::fmt;
use std::fs::File;
use std::io::Read;
use structopt::StructOpt;

#[derive(Serialize, Deserialize, Debug)]
pub struct Concern {
    contract: String,
    user: String,
}

#[derive(StructOpt, Deserialize, Debug)]
#[structopt(name = "basic")]
struct EnvCLIConfiguration {
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
    /// Special concern's contract
    #[structopt(long = "concern_contract")]
    concern_contract: Option<String>,
    /// Special concern's user address
    #[structopt(long = "concern_user")]
    concern_user: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct FileConfiguration {
    url: Option<String>,
    testing: Option<bool>,
    max_delay: Option<u64>,
    warn_delay: Option<u64>,
    concerns: Vec<Concern>,
}

#[derive(Debug)]
pub struct Configuration {
    pub url: String,
    pub testing: bool,
    pub max_delay: u64,
    pub warn_delay: u64,
    pub concerns: Vec<Concern>,
}

impl Configuration {
    pub fn new(path: &str) -> Result<Configuration> {
        // tries to load config from CLI arguments
        let cli_config = EnvCLIConfiguration::from_args();
        info!("CLI args: {:?}", cli_config);
        println!("Bla");
        // try to load config from env
        let env_config = envy::from_env::<EnvCLIConfiguration>()?;
        info!("Env args: {:?}", env_config);

        // try to read config from file
        let mut file = File::open(path).chain_err(|| {
            format!("unable to read configuration file: {}", path)
        })?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).chain_err(|| {
            format!("could not read from configuration file: {}", path)
        })?;
        let file_config: FileConfiguration =
            serde_yaml::from_str(&contents[..])
                .map_err(|e| error::Error::from(e))
                .chain_err(|| {
                    format!("could not parse configuration file: {}", path)
                })?;

        // determining url (cli -> env -> config)
        let url: String = cli_config
            .url
            .or(env_config.url)
            .or(file_config.url)
            .ok_or(Error::from(ErrorKind::InvalidConfig(String::from(
                "Need to provide url (config file, command line or env)",
            ))))?;

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

        // determine cli concern
        let cli_concern_defined = cli_config.concern_contract.is_some()
            || cli_config.concern_user.is_some();
        let cli_concern = if cli_concern_defined {
            Some(Concern {
                contract: cli_config
                    .concern_contract
                    .expect("All fields of cli_concern should be specified"),
                user: cli_config
                    .concern_user
                    .expect("All fields of cli_concern should be specified"),
            })
        } else {
            None
        };

        // determine env concern
        let env_concern_defined = env_config.concern_contract.is_some()
            || env_config.concern_user.is_some();
        let env_concern = if env_concern_defined {
            Some(Concern {
                contract: env_config
                    .concern_contract
                    .expect("All fields of env_concern should be specified"),
                user: env_config
                    .concern_user
                    .expect("All fields of env_concern should be specified"),
            })
        } else {
            None
        };

        // determine concerns
        let mut concerns = file_config.concerns;
        if let Some(c) = cli_concern {
            concerns.push(c);
        };
        if let Some(c) = env_concern {
            concerns.push(c);
        };

        let config = Configuration {
            url: url,
            testing: testing,
            max_delay: max_delay,
            warn_delay: warn_delay,
            concerns: concerns,
        };

        info!("Combined args: {:?}", config);

        if config.max_delay < config.warn_delay {
            return Err(Error::from(ErrorKind::InvalidConfig(
                "max_delay should be larger than warn delay".into(),
            )));
        }

        //println!("{}", config);
        Ok(config)
    }
}

impl fmt::Display for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{ Url: {}\
             Testing: {}\
             Max delay: {}\
             Warning delay: {}\
             Number of concerns: {} }}",
            self.url,
            self.testing,
            self.max_delay,
            self.warn_delay,
            self.concerns.len()
        )
    }
}
