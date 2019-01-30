#![feature(proc_macro_hygiene, generators, transpose_result)]

extern crate configuration;
extern crate env_logger;
extern crate error;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate db_key;
extern crate ethabi;
extern crate ethcore_transaction;
extern crate ethereum_types;
extern crate futures_await as futures;
extern crate leveldb;
extern crate serde_json;
extern crate utils;
extern crate web3;

use configuration::{Concern, Configuration};
use error::*;
use ethabi::{Param, Token};
use ethcore_transaction::{Action, Transaction};
use ethereum_types::{Address, U256};
use futures::prelude::{async_block, await};
use leveldb::database::Database;
use leveldb::iterator::Iterable;
use leveldb::kv::KV;
use leveldb::options::{ReadOptions, WriteOptions};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;
use utils::EthWeb3;
use web3::contract::Options;
use web3::futures::future::Either;
use web3::futures::stream;
use web3::futures::Future;
use web3::futures::Stream;
use web3::types::{Bytes, CallRequest};

use web3::contract::tokens::Tokenize;

#[derive(Clone, Debug)]
pub struct Instance {
    pub concern: Concern,
    pub index: U256,
    pub json_data: String,
    pub sub_instances: Vec<Box<Instance>>,
}

struct ConcernData {
    contract: Rc<web3::contract::Contract<web3::transports::http::Http>>,
    abi: Rc<ethabi::Contract>,
    file_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ConcernCache {
    last_maximum_index: usize,
    list_instances: Vec<usize>,
}

pub struct StateManager {
    config: Configuration,
    web3: web3::Web3<web3::transports::http::Http>,
    _eloop: web3::transports::EventLoopHandle, // kept to stay in scope
    concern_data: HashMap<Concern, ConcernData>,
    database: Rc<Database<Concern>>,
}

impl StateManager {
    pub fn new(
        config: Configuration //web3: web3::Web3<web3::transports::http::Http>
    ) -> Result<StateManager> {
        info!("Opening state manager database");
        let mut options = leveldb::options::Options::new();
        // if no database is found we start an empty one (no cache)
        options.create_if_missing = true;
        let mut database: Database<Concern> =
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

        info!("Preparing data for {} concerns", config.concerns.len());
        let mut concern_data = HashMap::new();
        for concern in config.clone().concerns {
            let abi_path = &config.abis.get(&concern).unwrap().abi;
            trace!(
                "Getting contract {} abi from file {:?}",
                &concern.contract_address,
                &abi_path
            );
            // change this to proper file handling (duplicate code in transact.)
            let mut file = File::open(abi_path)?;
            let mut s = String::new();
            let truffle_abi = file.read_to_string(&mut s)?;
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
                    contract: Rc::new(contract),
                    abi: Rc::new(abi),
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
            config: config,
            concern_data: concern_data,
            web3: web3,
            _eloop: _eloop,
            database: Rc::new(database),
        })
    }

    /// Gets the information about a given concern as it was stored in db
    fn get_concern_cache(&self, ref concern: &Concern) -> Result<ConcernCache> {
        let database = Rc::clone(&self.database);
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

    /// Gets an expanded version of the instances by querying the blockchain
    fn get_expanded_cache(
        &self,
        concern: Concern,
    ) -> Box<Future<Item = Rc<ConcernCache>, Error = Error>> {
        let contract = Rc::clone(
            &match self.concern_data.get(&concern) {
                Some(k) => k,
                None => {
                    return Box::new(async_block! {
                    Err(Error::from(ErrorKind::InvalidStateRequest(
                        String::from("Concern requested not found"),
                    )))});
                }
            }
            .contract,
        );
        let mut concern_cache = match self.get_concern_cache(&concern) {
            Ok(concern) => concern,
            Err(e) => {
                return Box::new(futures::future::err(
                    Error::from(e)
                        .chain_err(|| "error while getting concern_cache"),
                ));
            }
        };
        trace!("Cached concern is {:?}", concern_cache);

        let a = contract
            .query("currentIndex", (), None, Options::default(), None)
            .map(|index: U256| index.as_usize())
            .map_err(|e| {
                Error::from(e).chain_err(|| "error while getting current index")
            })
            .wait();

        trace!("Querying current index in contract {}", contract.address());
        let current_index = contract
            .query("currentIndex", (), None, Options::default(), None)
            .map(|index: U256| index.as_usize())
            .map_err(|e| {
                Error::from(e).chain_err(|| "error while getting current index")
            });

        let cached_index = concern_cache.last_maximum_index;

        return Box::new(current_index.and_then(move |index| {
            trace!(
                "Number of instances in contract {} is {}",
                contract.address(),
                index
            );

            if index > cached_index {
                trace!("Adding extra indices to the list");
                // all new indices become a future queryint its concern
                let mut instance_futures = vec![];
                for index in cached_index..index {
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
                            Rc::new(ConcernCache {
                                last_maximum_index: index,
                                list_instances: concern_cache.list_instances,
                            })
                        })
                        .map_err(|e| {
                            Error::from(e)
                                .chain_err(|| "error while filtering instances")
                        }),
                )
            } else {
                Either::B(futures::future::ok(Rc::new(ConcernCache {
                    last_maximum_index: index,
                    list_instances: concern_cache.list_instances,
                })))
            }
        }));
    }

    /// Get all instances, by querying cache, blockchain and then filtering
    pub fn get_instances(
        &self,
        concern: Concern,
    ) -> Box<Future<Item = Vec<usize>, Error = Error>> {
        let contract = Rc::clone(
            &match self.concern_data.get(&concern) {
                Some(k) => k,
                None => {
                    return Box::new(async_block! {
                    Err(Error::from(ErrorKind::InvalidStateRequest(
                        String::from("Concern requested not found"),
                    )))});
                }
            }
            .contract,
        );

        let expanded_cache = self
            .get_expanded_cache(concern)
            .map_err(|e| e.chain_err(|| "error while getting expanded cache"));
        let database = Rc::clone(&self.database);

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
                        (U256::from(i)),
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
            // create a stream of the above list of futures as they resolve
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
    ) -> Box<Future<Item = Instance, Error = Error>> {
        // this first implementation is completely synchronous
        // since we wait for all internal futures and then return
        // an imediatly available future. But we should refactor this
        // using futures::future::loop_fn to interactively explor th
        // tree of instances.
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
            "Retrieving contract and abi for {} ({})",
            concern_data.file_name,
            concern_data.contract.address(),
        );
        let contract = Rc::clone(&concern_data.contract);
        let abi = Rc::clone(&concern_data.abi);

        let function = match abi.function("getState".into()) {
            Ok(s) => s,
            Err(e) => return Box::new(futures::future::err(Error::from(e))),
        };

        let call = match function.encode_input(&U256::from(index).into_tokens())
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
                    data: Some(Bytes(call)),
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

        let json_data = match state.wait() {
            Ok(s) => s,
            Err(e) => return Box::new(futures::future::err(Error::from(e))),
        };

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
            println!("{:?}", instance.clone());

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
