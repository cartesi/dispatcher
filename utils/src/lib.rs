#![feature(generators, proc_macro_hygiene)]

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
use web3::types::{BlockId, BlockNumber};
use web3::Transport;

pub trait EthExt<T: Transport> {
    fn get_delay(self) -> Box<Future<Item = i64, Error = Error>>;
}

impl<T: Transport + 'static> EthExt<T> for web3::api::Eth<T> {
    #[async(boxed)]
    fn get_delay(self) -> Result<i64> {
        let block = await!(self.block(BlockId::Number(BlockNumber::Latest)))?;
        let block_time: i64 = block
            .ok_or(Error::from(ErrorKind::ChainError(
                "Latest block not found".to_string(),
            )))?
            .timestamp
            .as_u64() as i64;
        let current_time: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_e| {
                Error::from(ErrorKind::ChainError(
                    "Time went backwords".to_string(),
                ))
            })?
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
        let web3_clone = self.web3().clone();
        Box::new(async_block! {
            await!(
                web3_clone
                .client_version()
                .map_err(move |_e| Error::from(
                    ErrorKind::ChainError(format!(
                       "no Ethereum node responding at url: {}",
                       url
                    ))))
                .map(|_| ())
            )
        })
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
                // got an intermediate delay
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

pub fn print_error(e: &Error) {
    error!("error: {}", e);

    for e in e.iter().skip(1) {
        error!("caused by: {}", e);
    }

    // The backtrace is not always generated. Try to run this example
    // with `RUST_BACKTRACE=1`.
    if let Some(backtrace) = e.backtrace() {
        error!("backtrace: {:?}", backtrace);
    }
}
