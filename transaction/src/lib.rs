#![feature(proc_macro_hygiene, generators)]

extern crate configuration;
extern crate env_logger;
extern crate envy;
extern crate error;

extern crate structopt;
#[macro_use]
extern crate log;
extern crate ethabi;
extern crate ethcore_transaction;
extern crate ethereum_types;
extern crate ethjson;
extern crate ethkey;
extern crate futures_await as futures;
extern crate hex;
extern crate keccak_hash;
extern crate rlp;
extern crate serde_json;
extern crate web3;

use configuration::{Concern, Configuration};
use error::*;
use ethabi::Token;
use ethcore_transaction::{Action, Transaction};
use ethereum_types::{Address, U256};
use ethkey::KeyPair;
use futures::prelude::{async_block, await};
use keccak_hash::keccak;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::time;
use web3::futures::Future;
use web3::types::Bytes;

#[derive(Clone, Debug)]
pub enum Strategy {
    Simplest,
}

#[derive(Clone, Debug)]
pub struct TransactionRequest {
    pub concern: configuration::Concern,
    pub value: U256,
    pub function: String,
    pub data: Vec<Token>,
    pub strategy: Strategy,
}

struct ConcernData {
    key_pair: KeyPair,
    abi: Arc<ethabi::Contract>,
}

pub struct TransactionManager {
    config: Configuration,
    concern_data: HashMap<Concern, ConcernData>,
    web3: Arc<web3::Web3<web3::transports::Http>>,
}

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// we need to implement recovering keys in keystore
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
fn recover_key(ref concern: &Concern) -> Result<KeyPair> {
    let key_string: String =
        std::env::var("CARTESI_CONCERN_KEY").chain_err(|| {
            format!(
                "for now, keys must be provided as env variable, provide one"
            )
        })?;
    info!("recovering key from environment variable");
    let key_pair = KeyPair::from_secret(
        key_string
            .trim_start_matches("0x")
            .parse()
            .chain_err(|| format!("failed to parse key"))?,
    )?;
    if key_pair.address() != concern.user_address {
        Err(Error::from(ErrorKind::InvalidTransactionRequest(
            format!("key not found for concern: {:?}", *concern).to_string(),
        )))
    } else {
        Ok(key_pair)
    }
}

impl TransactionManager {
    pub fn new(
        config: Configuration,
        web3: web3::Web3<web3::transports::Http>,
    ) -> Result<TransactionManager> {
        let mut concern_data = HashMap::new();
        for concern in config.clone().concerns {
            trace!("Retrieving key for concern {}", &concern.contract_address);
            let key = recover_key(&concern)?;

            let abi_path = &config.abis.get(&concern).unwrap().abi;
            trace!(
                "Getting contract {} abi from file {:?}",
                &concern.contract_address,
                &abi_path
            );
            // change this to proper file handling (duplicate code in state)
            let mut file = File::open(abi_path)?;
            let mut s = String::new();
            let truffle_abi = file.read_to_string(&mut s)?;
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

    pub fn send(
        &self,
        request: TransactionRequest,
    ) -> Box<Future<Item = (), Error = Error>> {
        // async_block needs owned values, so let us clone some stuff
        let web3 = Arc::clone(&self.web3);
        let request = request.clone();
        let concern_data = match self.concern_data.get(&request.concern) {
            Some(k) => k,
            None => {
                return Box::new(async_block! {
                Err(Error::from(ErrorKind::InvalidTransactionRequest(
                    String::from("Concern requested not found"),
                )))});
            }
        };
        let key = concern_data.key_pair.clone();
        let abi = concern_data.abi.clone();
        let confirmations: usize = (&self).config.confirmations;

        Box::new(async_block! {
            let nonce = await!(web3.eth()
                               .transaction_count(key.address(), None)
            ).chain_err(|| "could not retrieve nonce")?;
            info!("Nonce for {} is {}", key.address(), nonce);
            let gas_price = await!(web3.eth().gas_price())
                .chain_err(|| "could not retrieve gas price")?;

            let raw_data = abi
                .function((&request.function[..]).into())
                .and_then(|function| {
                    function.encode_input(&request.data)
                }).chain_err(|| format!(
                    "could not encode data {:?} to function {}:",
                    &request.data,
                    &request.function
                ))?;

            info!("Gas price estimated as {}", gas_price);
            let call_request = web3::types::CallRequest {
                from: Some(key.address()),
                to: request.concern.contract_address,
                gas: None,
                gas_price: None,
                value: Some(request.value),
                data: Some(Bytes(raw_data.clone())),
            };
            let gas = await!(web3.eth().estimate_gas(call_request, None))
                .chain_err(|| format!("could not estimate gas usage"))?;
            info!("Gas usage estimated as {}", gas);
            info!("Signing transaction");
            let signed_tx = Transaction {
                action: Action::Call(request.concern.contract_address),
                nonce: nonce,
                // do something better then double
                gas_price: U256::from(2).checked_mul(gas_price).unwrap(),
                // do something better then double
                gas: U256::from(2).checked_mul(gas).unwrap(),
                value: request.value,
                data: raw_data,
            }
            .sign(&key.secret(), Some(69));
            info!("Sending transaction");
            let raw = Bytes::from(rlp::encode(&signed_tx));
            //let hash = await!(web3.eth().send_raw_transaction(raw));
            let poll_interval = time::Duration::from_secs(1);
            let hash = await!(
                web3::confirm::send_raw_transaction_with_confirmation(
                    web3.transport().clone(),
                    raw,
                    poll_interval,
                    confirmations,
                )
            );
            info!("Transaction sent with hash: {:?}", hash?);
            assert_eq!(Address::from(keccak(key.public())), signed_tx.sender());
            assert_eq!(signed_tx.chain_id(), Some(69));
            Ok(())
        })
    }
}
