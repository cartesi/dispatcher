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

#[macro_use]
extern crate log;
extern crate configuration;
extern crate env_logger;
extern crate error;
extern crate time;
extern crate web3;

pub use error::*;
use std::time::{SystemTime, UNIX_EPOCH};
use time::Duration;
use web3::futures::Future;
use web3::types::{BlockId, BlockNumber};
use web3::Transport;

pub trait EthExt<T: Transport> {
    fn get_delay(self) -> Box<dyn Future<Item = i64, Error = Error>>;
}

impl<T: Transport + 'static> EthExt<T> for web3::api::Eth<T> {
    fn get_delay(self) -> Box<dyn Future<Item = i64, Error = Error>> {
        Box::new(
            self.block(BlockId::Number(BlockNumber::Latest))
                .and_then(|block| {
                    block.ok_or(web3::Error::from(
                        "Latest block not found".to_string(),
                    ))
                })
                .map_err(|_| {
                    Error::from(ErrorKind::ChainError(
                        "Latest block not found".to_string(),
                    ))
                })
                .and_then(|block| {
                    let block_time: i64 = block.timestamp.as_u64() as i64;
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|_e| {
                            Error::from(ErrorKind::ChainError(
                                "Time went backwords".to_string(),
                            ))
                        })
                        .map(|duration| {
                            let current_time = duration.as_secs() as i64;
                            current_time - block_time
                        })
                }),
        )
    }
}

pub trait EthWeb3<T: Transport> {
    fn test_connection(
        &self,
        &configuration::Configuration,
    ) -> Box<dyn Future<Item = (), Error = Error>>;
    fn node_in_sync(
        &self,
        &configuration::Configuration,
    ) -> Box<dyn Future<Item = (), Error = Error>>;
}

impl<T: Transport + 'static> EthWeb3<T> for web3::Web3<T> {
    fn test_connection(
        &self,
        config: &configuration::Configuration,
    ) -> Box<dyn Future<Item = (), Error = Error>> {
        info!("Testing Ethereum node's responsiveness");
        let url = config.url.clone();
        let web3_clone = self.web3().clone();
        Box::new(
            web3_clone
                .client_version()
                .map_err(move |_e| {
                    Error::from(ErrorKind::ChainError(format!(
                        "no Ethereum node responding at url: {}",
                        url
                    )))
                })
                .map(|_| ()),
        )
    }

    fn node_in_sync(
        &self,
        ref config: &configuration::Configuration,
    ) -> Box<dyn Future<Item = (), Error = Error>> {
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
