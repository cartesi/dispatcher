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
extern crate web3;

use configuration::{Concern, Configuration};
use error::*;
use ethabi::Token;
use ethcore_transaction::{Action, Transaction};
use ethereum_types::{Address, U256};
use futures::prelude::{async_block, await};
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;
use web3::contract::Options;
use web3::futures::future::Either;
use web3::futures::stream;
use web3::futures::Future;
use web3::futures::Stream;
use web3::types::{Bytes, CallRequest};

use leveldb::database::Database;
use leveldb::iterator::Iterable;
use leveldb::kv::KV;
use leveldb::options::{ReadOptions, WriteOptions};

#[derive(Clone, Debug)]
pub struct Instance {
    contract_address: Address,
    index: U256,
    sub_instances: Box<Vec<Instance>>,
}

struct ConcernData {
    contract: Rc<web3::contract::Contract<web3::transports::http::Http>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ConcernCache {
    last_maximum_index: usize,
    list_instances: Vec<usize>,
}

pub struct StateManager {
    config: Configuration,
    web3: Rc<web3::Web3<web3::transports::http::Http>>,
    concern_data: HashMap<Concern, ConcernData>,
    database: Rc<Database<Concern>>,
}

impl StateManager {
    pub fn new(
        config: Configuration,
        web3: web3::Web3<web3::transports::http::Http>,
    ) -> Result<StateManager> {
        info!("Getting contract's abi from truffle");
        // change this to proper file handling
        let truffle_dump = include_str!(
            "/home/augusto/contracts/build/contracts/Instantiator.json"
        );
        let v: Value = serde_json::from_str(truffle_dump)
            .chain_err(|| format!("could not read truffle json file"))?;

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

        info!("Preparing data for {} concerns", config.concerns.len());
        let mut concern_data = HashMap::new();
        for concern in config.clone().concerns {
            // create a contract object
            let contract = web3::contract::Contract::from_json(
                web3.eth().clone(),
                concern.contract_address,
                serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
            )
            .chain_err(|| format!("could not decode json abi"))?;

            // store concern data in hash table
            concern_data.insert(
                concern.clone(),
                ConcernData {
                    contract: Rc::new(contract),
                },
            );
        }

        Ok(StateManager {
            config: config,
            concern_data: concern_data,
            web3: Rc::new(web3),
            database: Rc::new(database),
        })
    }

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
            Err(e) => return Box::new(futures::future::err(Error::from(e))),
        };
        trace!("Cached concern is {:?}", concern_cache);

        trace!("Querying current index in contract {}", contract.address());
        let current_index = contract
            .query("currentIndex", (), None, Options::default(), None)
            .map(|index: U256| index.as_usize())
            .map_err(|e| Error::from(e));

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
                            .map_err(|e| Error::from(e)),
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

        let expanded_cache = self.get_expanded_cache(concern);
        let database = Rc::clone(&self.database);

        Box::new(expanded_cache.and_then(move |cache| {
            let cache_list = cache.list_instances.clone();
            trace!("Removing inactive instances");
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
                    .map_err(|e| Error::from(e))
            });

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
        }))
    }

    pub fn get_state(
        &self,
        contract_address: Address,
        index: usize,
    ) -> Box<Future<Item = Vec<Instance>, Error = Error>> {
        let instance = Instance {
            contract_address: contract_address,
            index: U256::from(index),
            sub_instances: Box::new(vec![]),
        };

        Box::new(
            //filtered_cache.and_then(move |cache| {
            // let  = cache_list.iter().map(|index| {
            //     let i = index.clone();
            //     contract
            //         .query(
            //             "isActive",
            //             (U256::from(i)),
            //             None,
            //             Options::default(),
            //             None,
            //         )
            //         .map(move |active: bool| (i, active))
            //         .map_err(|e| Error::from(e))
            // });
            //}
            futures::future::ok(vec![instance]),
        )
    }
}
