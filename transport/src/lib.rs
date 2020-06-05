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

extern crate error;

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
extern crate url;
extern crate web3;
extern crate serde_json;
extern crate jsonrpc_core;
extern crate tokio_timer;

use error::*;
use jsonrpc_core::Value;
use web3::futures::future;
use web3::futures::Future;
use std::time::Duration;
use tokio_timer::Timer;

/// Generic transport
#[derive(Debug, Clone)]
pub struct GenericTransport {
    http: Option<web3::transports::http::Http>,
    ws: Option<web3::transports::ws::WebSocket>,
    timeout: u64,
}

impl GenericTransport {
    pub fn new(connstr: &str, timeout: u64) -> Result<(web3::transports::EventLoopHandle, GenericTransport)> {
        let mut generic_transport = GenericTransport {
            http: None,
            ws: None,
            timeout: timeout,
        };

        match url::Url::parse(connstr)?.scheme() {
            "http" | "https" => {
                let transport = web3::transports::Http::new(&connstr[..])
                .chain_err(|| {
                    format!("could not connect to Eth node with Http")
                })?;
                generic_transport.http = Some(transport.1);
                info!("GenericTransport created successfully with underlying Http");
                return Ok((transport.0, generic_transport));
            },
            "ws" | "wss" => {
                let transport = web3::transports::WebSocket::new(&connstr[..]).chain_err(|| {
                    format!("could not connect to Eth node with WebSocket")
                })?;
                generic_transport.ws = Some(transport.1);
                info!("GenericTransport created successfully with underlying WebSocket");
                return Ok((transport.0, generic_transport));
            }
            _ => bail!(ErrorKind::InvalidConfig(
                "Need to provide a valid http(s)/ws url (config file, command line or env)"
                    .to_string(),
            )),
        }
    }
}

impl web3::Transport for GenericTransport {
    type Out = Box<dyn Future<Item = Value, Error = web3::error::Error> + Send + 'static>;
    fn send(&self, id: web3::RequestId, request: jsonrpc_core::Call) -> Self::Out {
        if let Some(s) = &self.http {
            return Box::new(s.send(id, request));
        }
        if let Some(s) = &self.ws {

            let duration = Duration::from_secs(self.timeout);
            let timer = Timer::default();
            let timeout = timer.sleep(duration);

            let timeout_send = s.send(id, request).select2(timeout)
                .then(|res| -> Self::Out {
                    match res {
                        Ok(future::Either::A((v, _))) => Box::new(
                                future::ok::<Value, web3::error::Error>(v)
                            ),
                        Ok(future::Either::B((_, _))) => Box::new(
                                future::err(
                                    web3::error::Error::Transport("timeout sending request.".to_string())
                                )
                            ),
                        Err(future::Either::A((e, _))) => Box::new(future::err(e)),
                        Err(future::Either::B((e, _))) => {
                            error!("{}", e);
                            Box::new(
                                future::err(
                                    web3::error::Error::Transport("timer error sending request.".to_string())
                                )
                            )
                        }
                    }
                });
            return Box::new(timeout_send);
        }

        return Box::new(
            web3::futures::future::err(
                web3::error::Error::Transport("Invalid transport type.".to_string())
            )
        );
    }
    fn prepare(&self, method: &str, params: Vec<Value>) -> (web3::RequestId, jsonrpc_core::Call) {
        if let Some(s) = &self.http {
            return s.prepare(method, params);
        }
        if let Some(s) = &self.ws {
            return s.prepare(method, params);
        }
        
        return (std::default::Default::default(),
            jsonrpc_core::Call::Invalid{
                id: jsonrpc_core::Id::Null});
    }
}