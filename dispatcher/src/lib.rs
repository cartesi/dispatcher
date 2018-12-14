extern crate configuration;
extern crate error;
extern crate utils;
extern crate web3;

#[macro_use]
extern crate log;
extern crate env_logger;

use configuration::Configuration;
pub use error::*;
use utils::EthExt;
use web3::futures::Future;

pub struct Dispatcher {
    configuration: Configuration,
    web3: web3::api::Web3<web3::transports::http::Http>,
}

impl Dispatcher {
    pub fn new(path: &str) -> Result<Dispatcher> {
        info!("Loading configuration file");
        let config = Configuration::new(path)?;

        info!("Trying to connect at {}", &config.url[..]);
        let (_eloop, transport) = web3::transports::Http::new(&config.url[..])
            .chain_err(|| {
                format!("could not connect to Eth node at url: {}", &config.url)
            })?;
        let web3 = web3::Web3::new(transport);

        info!("Testing Ethereum node's responsiveness");
        web3.web3().client_version().wait().chain_err(|| {
            format!("no Ethereum node responding at url: {}", &config.url)
        })?;

        let ans = Dispatcher {
            configuration: config,
            web3: web3,
        };

        ans.delay_exceeded()?;

        Ok(ans)
    }

    pub fn delay_exceeded(&self) -> Result<()> {
        if self.configuration.testing {
            Ok(())
        } else {
            info!("Testing if Ethereum's node is up to date");

            let delay = self.web3.eth().get_delay().wait()?;

            // intermediate delay
            if (delay > self.configuration.warn_delay as i64)
                && (delay <= self.configuration.max_delay as i64)
            {
                warn!("ethereum node is delayed, but not above max_delay");
                return Ok(());
            }

            // exceeded max_delay
            if delay > self.configuration.max_delay as i64 {
                return Err(Error::from(ErrorKind::ChainNotInSync(
                    delay,
                    self.configuration.max_delay,
                )));
            }
            Ok(())
        }
    }
}
