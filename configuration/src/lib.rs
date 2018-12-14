extern crate error;

//extern crate ethabi;

const DEFAULT_MAX_DELAY: u64 = 500;
const DEFAULT_WARN_DELAY: u64 = 100;

use error::*;
use std::fmt;
use std::fs::File;
use std::io::Read;

#[macro_use]
extern crate serde_derive;

#[derive(Serialize, Deserialize, Debug)]
pub struct Concern {
    contract: String,
    user: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ParsedConfiguration {
    url: String,
    testing: Option<bool>,
    max_delay: Option<u64>,
    warn_delay: Option<u64>,
    concerns: Vec<Concern>,
}

pub struct Configuration {
    pub url: String,
    pub testing: bool,
    pub max_delay: u64,
    pub warn_delay: u64,
    pub concerns: Vec<Concern>,
}

impl Configuration {
    pub fn new(path: &str) -> Result<Configuration> {
        let mut file = File::open(path).chain_err(|| {
            format!("unable to read configuration file: {}", path)
        })?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).chain_err(|| {
            format!("could not read from configuration file: {}", path)
        })?;
        let parsed_config: ParsedConfiguration =
            serde_yaml::from_str(&contents[..])
                .map_err(|e| error::Error::from(e))
                .chain_err(|| {
                    format!("Could not parse configuration file: {}", path)
                })?;

        let config = Configuration {
            url: parsed_config.url,
            testing: parsed_config.testing.unwrap_or(false),
            max_delay: parsed_config.max_delay.unwrap_or(DEFAULT_MAX_DELAY),
            warn_delay: parsed_config.warn_delay.unwrap_or(DEFAULT_WARN_DELAY),
            concerns: parsed_config.concerns,
        };

        if config.max_delay < config.warn_delay {
            return Err(Error::from(ErrorKind::InvalidConfig));
        }
        Ok(config)
    }
}

impl fmt::Display for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Url: {}\n\
             Testing: {}\n\
             Max delay: {}\n\
             Warning delay: {}\n\
             Number of concerns: {}",
            self.url,
            self.testing,
            self.max_delay,
            self.warn_delay,
            self.concerns.len()
        )
    }
}
