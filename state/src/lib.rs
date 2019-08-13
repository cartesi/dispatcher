// Note: This component currently has dependencies that are licensed under the GNU GPL, version 3, and so you should treat this component as a whole as being under the GPL version 3. But all Cartesi-written code in this component is licensed under the Apache License, version 2, or a compatible permissive license, and can be used independently under the Apache v2 license. After this component is rewritten, the entire component will be released under the Apache v2 license.

// Copyright 2019 Cartesi Pte. Ltd.

// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.




//#![feature(transpose_result)]

extern crate configuration;
extern crate env_logger;
extern crate error;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate db_key;
extern crate ethabi;
//extern crate ethcore_transaction;
extern crate ethereum_types;
extern crate leveldb;
extern crate serde_json;
extern crate utils;
extern crate web3;

use configuration::{Concern, Configuration};
use error::*;
use ethabi::{Param, Token};
use ethereum_types::{Address, U256};
use leveldb::database::Database;
use leveldb::kv::KV;
use leveldb::options::{ReadOptions, WriteOptions};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use utils::EthWeb3;
use web3::contract::Options;
use web3::futures;
use web3::futures::future::err;
use web3::futures::future::ok;
use web3::futures::future::Either;
use web3::futures::stream;
use web3::futures::Future;
use web3::futures::Stream;
use web3::types::{Bytes, CallRequest};

use web3::contract::tokens::Tokenize;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instance {
    pub concern: Concern,
    pub index: U256,
    pub json_data: String,
    pub sub_instances: Vec<Box<Instance>>,
}

struct ConcernData {
    contract: Arc<web3::contract::Contract<web3::transports::http::Http>>,
    abi: Arc<ethabi::Contract>,
    file_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ConcernCache {
    last_maximum_index: usize,
    list_instances: Vec<usize>,
}

pub struct StateManager {
    web3: web3::Web3<web3::transports::http::Http>,
    _eloop: web3::transports::EventLoopHandle, // kept to stay in scope
    concern_data: HashMap<Concern, ConcernData>,
    database: Arc<Database<Concern>>,
}

impl StateManager {
    pub fn new(config: Configuration) -> Result<StateManager> {
        info!("Opening state manager database");
        let mut options = leveldb::options::Options::new();
        // if no database is found we start an empty one (no cache)
        options.create_if_missing = true;
        let database: Database<Concern> =
            Database::open(&config.working_path.join("state_db"), options)
                .chain_err(|| {
                    format!(
                    "no state database (use -i if running for the first time)"
                )
                })?;

        info!("Trying to connect to Eth node at {}", &config.url[..]);
        let (_eloop, transport) = web3::transports::Http::new(&config.url[..])
            .chain_err(|| {
                format!("could not connect to Eth node at url: {}", &config.url)
            })?;

        info!("Testing Ethereum node's functionality");
        let web3 = web3::Web3::new(transport);
        web3.test_connection(&config).wait()?;

        info!("Preparing assets for {} concerns", config.concerns.len());
        let mut concern_data = HashMap::new();
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

            // create a contract object
            let contract = web3::contract::Contract::from_json(
                web3.eth().clone(),
                concern.contract_address,
                serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
            )
            .chain_err(|| format!("could not decode json abi"))?;

            // create a low level abi for contract
            let abi = ethabi::Contract::load(
                serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
            )?;

            // store concern data in hash table
            trace!("Inserting concern {:?}", concern.clone());
            concern_data.insert(
                concern.clone(),
                ConcernData {
                    contract: Arc::new(contract),
                    abi: Arc::new(abi),
                    file_name: String::from(
                        abi_path
                            .clone()
                            .as_path()
                            .file_stem()
                            .unwrap()
                            .to_str()
                            .unwrap(),
                    ),
                },
            );
        }

        Ok(StateManager {
            concern_data: concern_data,
            web3: web3,
            _eloop: _eloop,
            database: Arc::new(database),
        })
    }

    /// Gets the information about a given concern as it was stored in db
    fn get_concern_cache(&self, ref concern: &Concern) -> Result<ConcernCache> {
        let database = Arc::clone(&self.database);
        trace!("Reading cached database for concern {:?}", concern);
        let read_opts = ReadOptions::new();
        Ok(database
            .get(read_opts, concern.clone())
            .chain_err(|| format!("could not read from state database"))?
            .map(|data: Vec<u8>| -> Result<ConcernCache> {
                let json_string: &str = std::str::from_utf8(&data)?;
                Ok(serde_json::from_str(json_string)?)
            })
            .transpose()
            .chain_err(|| format!("could not decode json from state db"))?
            .unwrap_or(ConcernCache {
                last_maximum_index: 0,
                list_instances: vec![],
            }))
    }

    /// Gets an expanded list of the instances by querying the blockchain
    fn get_expanded_cache(
        &self,
        concern: Concern,
    ) -> Box<Future<Item = Arc<ConcernCache>, Error = Error> + Send> {
        // first get the cached instances
        let mut concern_cache = match self.get_concern_cache(&concern) {
            Ok(concern) => concern,
            Err(e) => {
                return Box::new(err(Error::from(e)
                    .chain_err(|| "error while getting concern_cache")));
            }
        };
        trace!("Cached concerns are {:?}", concern_cache);

        // clone contract to move it to future clojure
        let contract = Arc::clone(
            &match self.concern_data.get(&concern) {
                Some(k) => k,
                None => {
                    return Box::new(err::<_, _>(Error::from(
                        ErrorKind::InvalidStateRequest(String::from(
                            "Concern requested not found",
                        )),
                    )));
                }
            }
            .contract,
        );
        trace!(
            "Querying current maximum index in contract {}",
            contract.address()
        );
        let current_max_index = contract
            .query("currentIndex", (), None, Options::default(), None)
            .map(|index: U256| index.as_usize())
            .map_err(|e| {
                Error::from(e).chain_err(|| "error while getting current index")
            });

        let cached_max_index = concern_cache.last_maximum_index;

        return Box::new(current_max_index.and_then(move |max_index| {
            trace!(
                "Current maximum number of indices in contract {} is {}",
                contract.address(),
                max_index
            );
            // check if any other instance has been created since last cache
            if max_index > cached_max_index {
                trace!("Adding extra indices to the cache");
                // all new indices become a future, querying if it is a concern
                let mut instance_futures = vec![];
                for index in cached_max_index..max_index {
                    instance_futures.push(
                        contract
                            .query(
                                "isConcerned",
                                (U256::from(index), concern.user_address),
                                None,
                                Options::default(),
                                None,
                            )
                            .map(move |is_concerned| (index, is_concerned))
                            .map_err(|e| {
                                Error::from(e).chain_err(|| {
                                    "error while querying isConcerned"
                                })
                            }),
                    );
                }
                // join the vector of futures into a stream,
                // filter only the ones that conern us,
                // returning a future that resolves to a ConcernCache
                Either::A(
                    futures::stream::futures_unordered(instance_futures)
                        .filter_map(|(index, is_concerned)| {
                            Some(index).filter(|_| is_concerned)
                        })
                        .collect()
                        .map(move |mut list_concerns| {
                            list_concerns.sort();
                            list_concerns.dedup();
                            concern_cache.list_instances.extend(list_concerns);
                            Arc::new(ConcernCache {
                                last_maximum_index: max_index,
                                list_instances: concern_cache.list_instances,
                            })
                        })
                        .map_err(|e| {
                            Error::from(e)
                                .chain_err(|| "error while filtering instances")
                        }),
                )
            } else {
                Either::B(ok(Arc::new(ConcernCache {
                    last_maximum_index: max_index,
                    list_instances: concern_cache.list_instances,
                })))
            }
        }));
    }

    /// Get relevant indices, by querying cache, blockchain and then filtering
    pub fn get_indices(
        &self,
        concern: Concern,
    ) -> Box<Future<Item = Vec<usize>, Error = Error> + Send> {
        // query tentative instances
        let expanded_cache = self
            .get_expanded_cache(concern)
            .map_err(|e| e.chain_err(|| "error while getting expanded cache"));

        // clone database and contract to move them to future clojure
        let database = Arc::clone(&self.database);
        let contract = Arc::clone(
            &match self.concern_data.get(&concern) {
                Some(k) => k,
                None => {
                    return Box::new(err::<_, _>(Error::from(
                        ErrorKind::InvalidStateRequest(String::from(
                            "Concern requested not found",
                        )),
                    )));
                }
            }
            .contract,
        );
        return Box::new(expanded_cache.and_then(move |cache| {
            let cache_list = cache.list_instances.clone();
            trace!("Removing inactive instances");
            // for each instance in the list, build a future that resolves
            // to whether that instance is active
            let active_futures = cache_list.iter().map(|index| {
                let i = index.clone();
                contract
                    .query(
                        "isActive",
                        U256::from(i),
                        None,
                        Options::default(),
                        None,
                    )
                    .map(move |active: bool| (i, active))
                    .map_err(|e| {
                        Error::from(e)
                            .chain_err(|| "error while querying isActive")
                    })
            });
            // create a stream from the above list of futures as they resolve
            stream::futures_unordered(active_futures)
                .filter_map(move |(index, is_active)| {
                    Some(index).filter(|_| is_active)
                })
                .collect()
                .map(move |vector_of_indices| {
                    trace!("Active instances are: {:?}", vector_of_indices);

                    let concern_cache = ConcernCache {
                        last_maximum_index: cache.last_maximum_index,
                        list_instances: vector_of_indices.clone(),
                    };

                    trace!("Writing relevant instances to state database");
                    let write_opts = WriteOptions::new();
                    let value =
                        serde_json::to_string(&concern_cache).unwrap().clone();
                    database
                        .put(write_opts, concern, value.as_bytes())
                        .unwrap();
                    vector_of_indices
                })
        }));
    }

    pub fn get_instance(
        &self,
        concern: Concern,
        index: usize,
    ) -> Box<Future<Item = Instance, Error = Error> + Send> {
        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        // this first implementation is completely synchronous
        // since we wait for all internal futures and then return
        // an imediatly available future. But we should refactor this
        // using futures::future::loop_fn to interactively explor the
        // tree of instances.
        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        trace!(
            "Get concern data for contract {}, index {}",
            concern.contract_address,
            index
        );
        let concern_data = match &self.concern_data.get(&concern) {
            Some(s) => s,
            None => {
                return Box::new(futures::future::err(Error::from(
                    ErrorKind::InvalidStateRequest(String::from(format!(
                        "Concern requested {:?} not found",
                        concern.clone()
                    ))),
                )));
            }
        }
        .clone();

        trace!(
            "Retrieving contract and abi for {} address ({})",
            concern_data.file_name,
            concern_data.contract.address(),
        );
        let contract = Arc::clone(&concern_data.contract);
        let abi = Arc::clone(&concern_data.abi);

        // call contract function to get its current state
        let function = match abi.function("getState".into()) {
            Ok(s) => s,
            Err(e) => return Box::new(futures::future::err(Error::from(e))),
        };

        let args = match function.encode_input(&U256::from(index).into_tokens())
        {
            Ok(s) => s,
            Err(e) => return Box::new(futures::future::err(Error::from(e))),
        };

        let state = self
            .web3
            .eth()
            .call(
                CallRequest {
                    from: None.into(),
                    to: contract.address().clone(),
                    gas: None.into(),
                    gas_price: None.into(),
                    value: None.into(),
                    data: Some(Bytes(args)),
                },
                None.into(),
            )
            .and_then(|result| {
                let types = &function.outputs;
                let tokens = function.decode_output(&result.0).unwrap();

                assert_eq!(types.len(), tokens.len());

                let response: Vec<String> = types
                    .iter()
                    .zip(tokens.iter())
                    .map(serialize_param)
                    .collect();

                Ok(format!("[{}]", response.join(",\n")))
            });

        // get contract's json data
        let json_data = match state.wait() {
            Ok(s) => s,
            Err(e) => return Box::new(futures::future::err(Error::from(e))),
        };

        // get all the sub instances that the current instance depend on
        let (sub_address, sub_indices): (Vec<Address>, Vec<U256>) =
            match contract
                .query(
                    "getSubInstances",
                    U256::from(index),
                    None,
                    Options::default(),
                    None,
                )
                .wait()
            {
                Ok(s) => s,
                Err(e) => return Box::new(futures::future::err(Error::from(e))),
            };
        // vector of addresses and indices should have the same length
        assert_eq!(sub_address.len(), sub_indices.len());
        // get all sub instances in a vector
        let mut sub_instances: Vec<Box<Instance>> = vec![];
        let sub_instance_indices = sub_address.iter().zip(sub_indices.iter());
        for instance in sub_instance_indices {
            let c = Concern {
                contract_address: *instance.0,
                user_address: concern.user_address,
            };

            sub_instances.push(Box::new(
                self.get_instance(c, instance.1.as_usize())
                    .wait()
                    .unwrap()
                    .clone(),
            ));
        }

        // join the subinstances together to return the current instance
        let starting_instance = Instance {
            concern: concern,
            index: U256::from(index),
            json_data: json_data,
            // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            // include nonce
            // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            sub_instances: sub_instances,
        };

        Box::new(futures::future::ok(starting_instance))
    }
}

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// replace this by proper serialization
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

fn serialize_param(input: (&Param, &Token)) -> String {
    let (ty, to) = input;
    let serialized = serialize_token(to);
    format!(
        "{{ \"name\": \"{}\", \"type\": \"{}\", \"value\": {} }}",
        ty.name, ty.kind, serialized
    )
}

fn serialize_token(to: &Token) -> String {
    match to {
        ethabi::Token::Address(a) => format!("\"{:?}\"", a),
        ethabi::Token::FixedBytes(f) => {
            format!("\"0x{}\"", Token::FixedBytes(f.to_vec()))
        }
        ethabi::Token::Bytes(b) => {
            format!("\"0x{}\"", Token::Bytes(b.to_vec()))
        }
        ethabi::Token::Int(_) => unimplemented!("Int"),
        ethabi::Token::Uint(u) => format!("\"0x{:x}\"", u),
        ethabi::Token::Bool(b) => format!("{}", b),
        ethabi::Token::String(_) => unimplemented!("String"),
        ethabi::Token::FixedArray(f) => {
            let temp = f
                .into_iter()
                .map(|token| serialize_token(token))
                .collect::<Vec<_>>()
                .join(",\n");
            format!("[{}]", temp) // go
        }
        ethabi::Token::Array(f) => {
            let temp = f
                .into_iter()
                .map(|token| serialize_token(token))
                .collect::<Vec<_>>()
                .join(",\n");
            format!("[{}]", temp) // go
        }
    }
}
