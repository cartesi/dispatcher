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

//! Server that sends transactions to the blockchain, dealing with
//! several issues, like estimating gas usage and waiting for
//! confirmations

extern crate configuration;
extern crate env_logger;
extern crate envy;
extern crate error;

extern crate structopt;
#[macro_use]
extern crate log;
extern crate common_types;
extern crate ethabi;
extern crate ethereum_types;
extern crate ethjson;
extern crate hex;
extern crate keccak_hash;
extern crate parity_crypto;
extern crate rlp;
extern crate serde_json;
extern crate transport;
extern crate web3;

use common_types::transaction::{Action, Transaction};
use configuration::{Concern, ConcernKey, Configuration};
use error::*;
use ethabi::Token;
use ethereum_types::U256;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use transport::GenericTransport;
use web3::futures::future::err;
use web3::futures::future::Either;
use web3::futures::Future;
use web3::types;
use web3::types::Bytes;

/// In the future there could be several strategies to submit a transaction.
/// Simplest is based on estimated gas cost.
#[derive(Clone, Debug)]
pub enum Strategy {
    Simplest,
}

/// The transaction manager expects these requests to be submitted to the
/// blockchain. Note that the data should have already been encoded,
/// since the trasaction manager does not understand ABI's.
#[derive(Clone, Debug)]
pub struct TransactionRequest {
    pub concern: configuration::Concern,
    pub value: U256,
    pub function: String,
    pub data: Vec<Token>,
    pub gas: Option<U256>,
    pub strategy: Strategy,
    pub contract_name: Option<String>,
}

/// Every concern that the Transaction Manager acts uppon should ether be
/// provided with a key pair to sign transactions, or with an address for
/// an external signer.
struct ConcernData {
    key: ConcernKey,
    abi: Arc<ethabi::Contract>,
}

/// A Transaction Manager server
pub struct TransactionManager {
    config: Configuration,
    concern_data: HashMap<Concern, ConcernData>,
    web3: Arc<web3::Web3<GenericTransport>>,
    // external_signer: bool,
}

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// We need to implement a function call to query
// whether a certain instance is being dealt with.
// This needs the instance's nonce to be considered
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
impl TransactionManager {
    /// Creates a new Transaction manager, based on the core's configuration
    pub fn new(
        config: Configuration,
        web3: web3::Web3<GenericTransport>,
    ) -> Result<TransactionManager> {
        let signer_user_address = config.clone().signer_user_address;

        let mut concern_data = HashMap::new();
        // loop through each concern, adding them to the concern's data
        for concern in config.clone().concerns {
            let abi_path = &config.abis.get(&concern).unwrap().abi;
            trace!(
                "Getting contract {} abi from file {:?}",
                &concern.contract_address,
                &abi_path
            );
            let mut file = File::open(abi_path)?;
            let mut s = String::new();
            file.read_to_string(&mut s)?;
            let v: Value = serde_json::from_str(&s[..])
                .chain_err(|| format!("could not read truffle json file"))?;

            // create a low level abi for contract
            let abi = ethabi::Contract::load(
                serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
            )?;

            concern_data.insert(
                concern,
                ConcernData {
                    key: signer_user_address.clone(),
                    abi: Arc::new(abi),
                },
            );
        }

        Ok(TransactionManager {
            config: config,
            concern_data: concern_data,
            web3: Arc::new(web3),
        })
    }

    /// Signs and sends a given transaction
    pub fn send(
        &self,
        request: TransactionRequest,
    ) -> Box<dyn Future<Item = (), Error = error::Error> + Send> {
        // async_block needs owned values, so let us clone some stuff
        let web3 = Arc::clone(&self.web3);
        let request_clone = request.clone();
        let request_concern = match request_clone.contract_name {
            None => request_clone.concern.clone(),
            Some(s) => match self.config.contracts.get(&s) {
                Some(k) => k.clone(),
                None => {
                    return Box::new(err(Error::from(
                        ErrorKind::InvalidTransactionRequest(String::from(
                            "Contract requested not found",
                        )),
                    )));
                }
            },
        };
        let concern_data = match self.concern_data.get(&request_concern) {
            Some(k) => k,
            None => {
                return Box::new(err(Error::from(
                    ErrorKind::InvalidTransactionRequest(String::from(
                        "Concern requested not found",
                    )),
                )));
            }
        };
        let request = request.clone();
        let key = concern_data.key.clone();
        let address = key.address();
        let abi = concern_data.abi.clone();
        let chain_id: u64 = (&self).config.chain_id;

        trace!("Getting nonce");
        let web3_gas_price = web3.clone();
        let web3_gas_usage = web3.clone();
        let request_gas_usage = request.clone();
        let request_to_address = request.clone();

        Box::new(
            web3.clone()
                .eth()
                .transaction_count(
                    address.clone(),
                    Some(web3::types::BlockNumber::Pending),
                )
                .map_err(|_e| {
                    error::Error::from(format!("could not retrieve nonce"))
                })
                .and_then(move |nonce| {
                    trace!("Estimating gas price");
                    web3_gas_price
                        .eth()
                        .gas_price()
                        .map_err(|_e| {
                            error::Error::from(format!(
                                "could not retrieve gas price"
                            ))
                        })
                        .map(move |gas_price| (nonce.clone(), gas_price))
                })
                .and_then(move |(nonce, gas_price)| {
                    info!(
                        "Nonce for {} is {}",
                        address.clone(),
                        nonce.clone()
                    );
                    trace!("Gas price estimated as {}", gas_price);
                    let raw_data_result = abi
                        .function((&request_gas_usage.function[..]).into())
                        .and_then(|function| {
                            function.encode_input(&request_gas_usage.data)
                        })
                        .chain_err(|| {
                            error::Error::from(format!(
                                "could not encode data {:?} to function {}:",
                                &request_gas_usage.data,
                                &request_gas_usage.function
                            ))
                        });
                    // Fix this unwrap below
                    let raw_data = raw_data_result.unwrap();
                    trace!("Buiding transaction");
                    let call_request = web3::types::CallRequest {
                        from: Some(address.clone()),
                        to: request_gas_usage.concern.contract_address,
                        gas: None,
                        gas_price: None,
                        value: Some(request_gas_usage.value),
                        data: Some(Bytes(raw_data.clone())),
                    };
                    trace!("Estimate total gas usage");
                    let request_string =
                        format!("{:?}", request_gas_usage.clone());

                    get_gas(web3_gas_usage, call_request, request_gas_usage)
                        .map_err(move |_e| {
                            error::Error::from(format!(
                                "could not estimate gas usage for call {:?}",
                                request_string
                            ))
                        })
                        .map(move |total_gas| {
                            (nonce, gas_price, total_gas, raw_data)
                        })
                })
                .and_then(move |(nonce, gas_price, total_gas, raw_data)| {
                    trace!("Gas usage estimated to be {}", total_gas);
                    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                    // Implement other sending strategies
                    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!


                    match key {
                        ConcernKey::KeyPair(key_pair) => {
                            trace!("Signing transaction");
                            let signed_tx = Transaction {
                                action: Action::Call(request_concern.contract_address),
                                nonce: nonce,
                                // do something better then double
                                gas_price: U256::from(2)
                                    .checked_mul(gas_price)
                                    .unwrap(),
                                // do something better then double
                                gas: U256::from(12).checked_mul(total_gas).unwrap(),
                                value: request.value,
                                data: raw_data,
                            }
                            .sign(&key_pair.secret(), Some(chain_id));

                            info!("Sending transaction: {:?}", &request);
                            let raw = Bytes::from(rlp::encode(&signed_tx));
                            //let hash = await!(web3.eth().send_raw_transaction(raw));

                            Either::A(web3.eth().send_raw_transaction(raw)
                                .map(|hash| {
                                    info!("Transaction sent with hash: {:?}", hash);
                                })
                                .or_else(|e| {
                                    // ignore the nonce error, by pass the other errors
                                    if let web3::error::Error::Rpc(ref rpc_error) = e {
                                        let nonce_error = String::from(
                                            "the tx doesn't have the correct nonce",
                                        );
                                        if rpc_error.message[..nonce_error.len()]
                                            == nonce_error
                                        {
                                            warn!(
                                                "Ignoring nonce Error: {}",
                                                rpc_error.message
                                            );
                                            return Box::new(web3::futures::future::ok::<(), _,> (
                                                ()
                                            ));
                                        }
                                    }
                                    return Box::new(web3::futures::future::err(e));
                                })
                                .map_err(|e| {
                                    warn!("Failed to send transaction. Error {}", e);
                                    error::Error::from(e)
                                }))
                        }

                        ConcernKey::UserAddress(address) => {
                            info!("Getting accounts from signer");
                            let tx_request =
                                types::TransactionRequest {
                                    from: address,
                                    to: Some(
                                        request_to_address
                                        .concern
                                        .contract_address,
                                    ),
                                    // do something better then double
                                    gas_price: Some(
                                        U256::from(2)
                                        .checked_mul(gas_price)
                                        .unwrap(),
                                    ),
                                    // do something better then double
                                    gas: Some(
                                        U256::from(2)
                                        .checked_mul(total_gas)
                                        .unwrap(),
                                    ),
                                    value: Some(request.value),
                                    data: Some(Bytes(raw_data.clone())),
                                    condition: None,
                                    nonce: Some(nonce),
                                };

                            info!("Sending unsigned transaction to signer: {:?}", &request);
                            Either::B(
                                web3.eth().send_transaction(tx_request)
                                .map(|hash| {
                                    info!("Transaction sent with hash: {:?}", hash);
                                })
                                .or_else(|e| {
                                    // ignore the nonce error, by pass the other errors
                                    if let web3::error::Error::Rpc(ref rpc_error) = e {
                                        let nonce_error = String::from(
                                            "the tx doesn't have the correct nonce",
                                        );
                                        if rpc_error.message[..nonce_error.len()]
                                            == nonce_error
                                        {
                                            warn!(
                                                "Ignoring nonce Error: {}",
                                                rpc_error.message
                                            );
                                            return Box::new(web3::futures::future::ok::<(),_,> (
                                                    ()
                                            ));
                                        }
                                    }
                                    return Box::new(web3::futures::future::err(e));
                                })
                                .map_err(|e| {
                                    warn!("Failed to send transaction. Error {}", e);
                                    error::Error::from(e)
                                })
                            )
                        }
                    }

                }),
        )
    }
}

fn get_gas(
    web3: Arc<web3::Web3<GenericTransport>>,
    call_request: web3::types::CallRequest,
    request: TransactionRequest,
) -> Box<dyn Future<Item = U256, Error = web3::Error> + Send> {
    match request.gas {
        Some(gas) => {
            return Box::new(web3::futures::future::ok::<U256, _>(gas));
        }
        None => {
            return Box::new(web3.eth().estimate_gas(call_request, None));
        }
    }
}
