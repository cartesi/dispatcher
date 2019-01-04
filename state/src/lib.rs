#![feature(proc_macro_hygiene, generators)]

extern crate configuration;
extern crate env_logger;
extern crate error;

#[macro_use]
extern crate log;
extern crate ethabi;
extern crate ethcore_transaction;
extern crate ethereum_types;
extern crate futures_await as futures;
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
use web3::futures::Future;
use web3::types::{Bytes, CallRequest};

#[derive(Clone)]
pub struct Issue {
    concern: Concern,
    index: U256,
    sub_issues: Box<Vec<Issue>>,
}

struct ConcernData {
    contract: Rc<web3::contract::Contract<web3::transports::http::Http>>,
}

pub struct StateManager {
    config: Configuration,
    //abi: Rc<ethabi::Contract>,
    web3: Rc<web3::Web3<web3::transports::http::Http>>,
    concern_data: HashMap<Concern, ConcernData>,
}

// fn query(
//     ref web3: &web3::Web3<web3::transports::Http>,
//     ref abi: &ethabi::Contract,
//     ref params: &Vec<Token>,
//     function: &str,
//     address: Address,
// ) -> Box<Future<Item = Value, Error = Error>> {
//     Box::new(
//         futures::future::result(
//             abi.function(function.into())
//                 .and_then(|function| {
//                     function.encode_input(&params).map(|call| (call, function))
//                 })
//                 .map(|(call, function)| {
//                     let result = web3.eth().call(
//                         CallRequest {
//                             from: None,
//                             to: address.clone(),
//                             gas: None,
//                             gas_price: None,
//                             value: None,
//                             data: Some(Bytes(call)),
//                         },
//                         None,
//                     );
//                     (result, function.clone())
//                 })
//         ).map(|(result, function)| {
//             QueryResult::new(result, function)
//         })

//         //          .unwrap_or_else(Into::into),
//     )
// }

impl StateManager {
    pub fn new(
        config: Configuration,
        web3: web3::Web3<web3::transports::http::Http>,
    ) -> Result<StateManager> {
        info!("Getting contract's abi from truffle");
        // change this to proper handling of file
        let truffle_dump = include_str!(
            "/home/augusto/contracts/build/contracts/Instantiator.json"
        );
        let v: Value = serde_json::from_str(truffle_dump)
            .chain_err(|| format!("could not read truffle json file"))?;
        let mut concern_data = HashMap::new();
        for concern in config.clone().concerns {
            let contract = web3::contract::Contract::from_json(
                web3.eth().clone(),
                concern.contract_address,
                serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
            )
            .chain_err(|| format!("could decode json abi"))?;
            concern_data.insert(
                concern,
                ConcernData {
                    contract: Rc::new(contract),
                },
            );
        }

        Ok(StateManager {
            config: config,
            concern_data: concern_data,
            web3: Rc::new(web3),
        })
    }

    pub fn get_issues(
        &self,
        concern: Concern,
    ) -> Box<Future<Item = (), Error = Error>> {
        //async_block needs owned values, so let us clone some stuff
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

        return Box::new(async_block! {
            info!("Querying current index in contract {}", contract.address());
            let current_index: U256 = await!(
                contract.query(
                    "currentIndex",
                    (),
                    None,
                    Options::default(),
                    None,
                ))?;
            info!("Number of issues is {}", current_index);

            Ok(())
        });
    }
}
