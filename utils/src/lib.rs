// Note: This component currently has dependencies that are licensed under the GNU GPL, version 3, and so you should treat this component as a whole as being under the GPL version 3. But all Cartesi-written code in this component is licensed under the Apache License, version 2, or a compatible permissive license, and can be used independently under the Apache v2 license. After this component is rewritten, the entire component will be released under the Apache v2 license.

// Copyright 2019 Cartesi Pte. Ltd.

// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.




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
    fn get_delay(self) -> Box<Future<Item = i64, Error = Error>>;
}

impl<T: Transport + 'static> EthExt<T> for web3::api::Eth<T> {
    fn get_delay(self) -> Box<Future<Item = i64, Error = Error>> {
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
