// Note: This component currently has dependencies that are licensed under the GNU GPL, version 3, and so you should treat this component as a whole as being under the GPL version 3. But all Cartesi-written code in this component is licensed under the Apache License, version 2, or a compatible permissive license, and can be used independently under the Apache v2 license. After this component is rewritten, the entire component will be released under the Apache v2 license.

// Copyright 2019 Cartesi Pte. Ltd.

// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.




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
extern crate ethabi;
extern crate common_types;
extern crate ethereum_types;
extern crate ethjson;
extern crate ethkey;
extern crate hex;
extern crate keccak_hash;
extern crate rlp;
extern crate serde_json;
extern crate web3;

use configuration::{Concern, Configuration};
use error::*;
use ethabi::Token;
use common_types::transaction::{Action, Transaction};
use ethereum_types::U256;
use ethkey::KeyPair;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::time;
use web3::futures::future::err;
use web3::futures::Future;
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
    pub strategy: Strategy,
}

/// Every concert that the Transaction Manager acts uppon should be
/// provided with a key pair to sign transactions.
struct ConcernData {
    key_pair: KeyPair,
    abi: Arc<ethabi::Contract>,
}

/// A Transaction Manager server
pub struct TransactionManager {
    config: Configuration,
    concern_data: HashMap<Concern, ConcernData>,
    web3: Arc<web3::Web3<web3::transports::Http>>,
}

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// we need to implement recovering keys in keystore
// the current method uses environmental variables
// and it is not safe enough
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
fn recover_key(ref concern: &Concern) -> Result<KeyPair> {
    info!("Recovering key from environment variable");
    let key_string: String =
        std::env::var("CARTESI_CONCERN_KEY").chain_err(|| {
            format!(
                "for now, keys must be provided as env variable, provide one"
            )
        })?;
    let key_pair = KeyPair::from_secret(
        key_string
            .trim_start_matches("0x")
            .parse()
            .chain_err(|| format!("failed to parse key"))?,
    )?;
    if key_pair.address() != concern.user_address {
        Err(Error::from(ErrorKind::InvalidTransactionRequest(
            format!("key does not match concern: {:?}", *concern).to_string(),
        )))
    } else {
        Ok(key_pair)
    }
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
        web3: web3::Web3<web3::transports::Http>,
    ) -> Result<TransactionManager> {
        let mut concern_data = HashMap::new();
        // loop through each concern, adding them to the concern's data
        for concern in config.clone().concerns {
            trace!("Retrieving key for concern {}", &concern.contract_address);
            let key = recover_key(&concern)
                .chain_err(|| "could not find key for concern")?;

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

            // store concern data in hash table
            trace!("Inserting concern {:?}", concern.clone());
            concern_data.insert(
                concern,
                ConcernData {
                    key_pair: key,
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
    ) -> Box<Future<Item = (), Error = error::Error> + Send> {
        // async_block needs owned values, so let us clone some stuff
        let web3 = Arc::clone(&self.web3);
        let request = request.clone();
        let concern_data = match self.concern_data.get(&request.concern) {
            Some(k) => k,
            None => {
                return Box::new(err(Error::from(
                    ErrorKind::InvalidTransactionRequest(String::from(
                        "Concern requested not found",
                    )),
                )));
            }
        };
        let key = concern_data.key_pair.clone();
        let abi = concern_data.abi.clone();
        let confirmations: usize = (&self).config.confirmations;

        trace!("Getting nonce");
        let web3_gas_price = web3.clone();
        let web3_gas_usage = web3.clone();
        let request_gas_usage = request.clone();
        let key_gas_price = key.clone();
        let key_gas_usage = key.clone();
        Box::new(
            web3.clone()
                .eth()
                .transaction_count(web3::types::H160::from_slice(&key.address().to_vec()[..]), None)
                .map_err(|e| {
                    error::Error::from(
                        e.chain_err(|| "could not retrieve nonce"),
                    )
                })
                .and_then(move |nonce| {
                    trace!("Estimating gas price");
                    web3_gas_price
                        .eth()
                        .gas_price()
                        .map_err(|e| {
                            error::Error::from(
                                e.chain_err(|| "could not retrieve gas price"),
                            )
                        })
                        .map(move |gas_price| (nonce.clone(), gas_price))
                })
                .and_then(move |(nonce, gas_price)| {
                    info!(
                        "Nonce for {} is {}",
                        key_gas_price.address().clone(),
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
                        from: Some(key_gas_usage.address()),
                        to: request_gas_usage.concern.contract_address,
                        gas: None,
                        gas_price: None,
                        value: Some(request_gas_usage.value),
                        data: Some(Bytes(raw_data.clone())),
                    };
                    trace!("Estimate total gas usage");
                    let request_string =
                        format!("{:?}", request_gas_usage.clone());
                    web3_gas_usage
                        .eth()
                        .estimate_gas(call_request, None)
                        .map_err(|e| {
                            error::Error::from(e.chain_err(move || {
                                format!(
                                "could not estimate gas usage for call {:?}",
                                request_string
                            )
                            }))
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

                    trace!("Signing transaction");
                    let signed_tx = Transaction {
                        action: Action::Call(request.concern.contract_address),
                        nonce: nonce,
                        // do something better then double
                        gas_price: U256::from(2)
                            .checked_mul(gas_price)
                            .unwrap(),
                        // do something better then double
                        gas: U256::from(2).checked_mul(total_gas).unwrap(),
                        value: request.value,
                        data: raw_data,
                    }
                    .sign(&key.secret(), Some(69));

                    info!("Sending transaction: {:?}", &request);
                    let raw = Bytes::from(rlp::encode(&signed_tx));
                    //let hash = await!(web3.eth().send_raw_transaction(raw));
                    let poll_interval = time::Duration::from_secs(1);
                    web3::confirm::send_raw_transaction_with_confirmation(
                        web3.transport().clone(),
                        raw,
                        poll_interval,
                        confirmations,
                    )
                    .map_err(|e| {
                        warn!("Failed to send transaction. Error {}", e);
                        error::Error::from(e)
                    })
                    .map(|hash| {
                        info!("Transaction sent with hash: {:?}", hash);
                    })
                }),
        )
    }
}
