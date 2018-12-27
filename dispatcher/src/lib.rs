extern crate configuration;
extern crate error;
extern crate utils;
extern crate web3;

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate transaction;

use configuration::Configuration;
pub use error::*;
use transaction::TransactionManager;
use utils::EthWeb3;
use web3::futures::Future;

pub struct Dispatcher {
    config: Configuration,
    web3: web3::api::Web3<web3::transports::http::Http>,
}

impl Dispatcher {
    pub fn new() -> Result<Dispatcher> {
        info!("Loading configuration file");
        let config = Configuration::new()?;

        info!("Trying to connect to Eth node at {}", &config.url[..]);
        let (_eloop, transport) = web3::transports::Http::new(&config.url[..])
            .chain_err(|| {
                format!("could not connect to Eth node at url: {}", &config.url)
            })?;
        info!("Connected to Eth node");
        let web3 = web3::Web3::new(transport);

        let ans = Dispatcher {
            config: config,
            web3: web3,
        };

        ans.web3.test_connection(&ans.config).wait()?;
        info!("Ethereum node responsive");
        // transaction

        let tm = TransactionManager::new(ans.config.clone());

        tm.send();

        Ok(ans)
    }
}
