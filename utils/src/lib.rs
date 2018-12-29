#![feature(proc_macro, generators)]

#[macro_use]
extern crate log;
extern crate configuration;
extern crate env_logger;
extern crate error;
extern crate futures_await as futures;
extern crate time;
extern crate web3;

pub use error::*;
use futures::prelude::await;
use futures::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};
use time::Duration;
use web3::futures::Future;
use web3::types::H256;
use web3::types::{Block, BlockId, BlockNumber};
use web3::Transport;

fn str_error(msg: &str) -> Error {
    ErrorKind::Msg(String::from(msg)).into()
}

pub trait EthExt<T: Transport> {
    fn get_delay(self) -> Box<Future<Item = i64, Error = Error>>;
}

impl<T: Transport + 'static> EthExt<T> for web3::api::Eth<T> {
    #[async(boxed)]
    fn get_delay(self) -> Result<i64> {
        let block = await!(self.block(BlockId::Number(BlockNumber::Latest)))?;
        let block_time: i64 = block
            .ok_or(str_error("Latest block not found"))?
            .timestamp
            .low_u64() as i64;
        let current_time: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_e| str_error("Time went backwards"))?
            .as_secs() as i64;
        Ok(current_time - block_time)
    }
}

pub trait EthWeb3<T: Transport> {
    fn test_connection(
        &self,
        &configuration::Configuration,
    ) -> Box<Future<Item = (), Error = Error>>;
    fn node_in_sync(
        &self,
        &configuration::Configuration,
    ) -> Box<Future<Item = (), Error = Error>>;
}

impl<T: Transport + 'static> EthWeb3<T> for web3::Web3<T> {
    fn test_connection(
        &self,
        config: &configuration::Configuration,
    ) -> Box<Future<Item = (), Error = Error>> {
        info!("Testing Ethereum node's responsiveness");
        let url = config.url.clone();
        Box::new(
            self.web3()
                .client_version()
                .map_err(move |_e| {
                    str_error(&format!(
                        "no Ethereum node responding at url: {}",
                        url
                    ))
                })
                .map(|_| ()),
        )
    }

    fn node_in_sync(
        &self,
        ref config: &configuration::Configuration,
    ) -> Box<Future<Item = (), Error = Error>> {
        if config.testing {
            info!("Testing if Ethereum's node is up to date");
            let warn_delay = config.warn_delay.clone();
            let max_delay = config.max_delay.clone();
            Box::new(self.eth().get_delay().then(move |res| {
                let delay = Duration::seconds(res?);
                // intermediate delay
                if (delay > warn_delay) && (delay <= max_delay) {
                    warn!("ethereum node is delayed, but not above max_delay");
                    return Ok(());
                }

                // exceeded max_delay
                if delay > max_delay {
                    return Err(Error::from(ErrorKind::ChainNotInSync(
                        delay, max_delay,
                    )));
                }
                Ok(())
            }))
        } else {
            Box::new(web3::futures::future::ok(()))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
