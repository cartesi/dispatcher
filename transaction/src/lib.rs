#![feature(proc_macro_hygiene, generators)]

extern crate configuration;
extern crate env_logger;
extern crate envy;
extern crate error;

extern crate structopt;
#[macro_use]
extern crate log;
extern crate ethcore_transaction;
extern crate ethereum_types;
extern crate ethjson;
extern crate ethkey;
extern crate futures_await as futures;
extern crate hex;
extern crate keccak_hash;
extern crate rlp;
extern crate web3;

use configuration::{Concern, Configuration};
use error::*;
use ethcore_transaction::{Action, Transaction};
use ethereum_types::{Address, U256};
use ethkey::KeyPair;
use futures::prelude::{async_block, await};
use keccak_hash::keccak;
use std::collections::HashMap;
use std::rc::Rc;
use web3::futures::Future;
use web3::types::Bytes;

#[derive(Clone)]
pub enum Strategy {
    Simplest,
}

#[derive(Clone)]
pub struct TransactionRequest {
    pub concern: configuration::Concern,
    pub value: U256,
    pub data: Vec<u8>,
    pub strategy: Strategy,
}

struct ConcernData {
    key_pair: KeyPair,
}

pub struct TransactionManager {
    config: Configuration,
    concern_data: HashMap<Concern, ConcernData>,
    web3: Rc<web3::Web3<web3::transports::Http>>,
    _eloop: web3::transports::EventLoopHandle, // needs to stay in scope
}

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// we need to implement recovering keys in keystore
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
fn recover_key(ref concern: &Concern) -> Result<KeyPair> {
    let key_string: String = std::env::var("CONCERN_KEY").chain_err(|| {
        format!("for now, keys must be provided as env variable, provide one")
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
    pub fn new(config: Configuration) -> Result<TransactionManager> {
        let url = config.url.clone();

        let mut concern_data = HashMap::new();
        for concern in config.clone().concerns {
            let key = recover_key(&concern)?;
            concern_data.insert(concern, ConcernData { key_pair: key });
        }

        info!("Trying to connect to Eth node at {}", &url[..]);
        let (_eloop, transport) = web3::transports::Http::new(&url[..])
            .chain_err(|| {
                format!("could not connect to Eth node at url: {}", &url[..])
            })?;
        let web3 = Rc::new(web3::Web3::new(transport));

        Ok(TransactionManager {
            config: config,
            concern_data: concern_data,
            web3: web3,
            _eloop: _eloop,
        })
    }

    pub fn send(
        &self,
        input_request: TransactionRequest,
    ) -> Box<Future<Item = (), Error = Error>> {
        // async_block needs owned values, so let us clone some stuff
        let web3 = Rc::clone(&self.web3);
        let request = input_request.clone();
        let key = match self.concern_data.get(&request.concern) {
            Some(k) => k,
            None => {
                return Box::new(async_block! {
                Err(Error::from(ErrorKind::InvalidTransactionRequest(
                    String::from("Concern requested not found"),
                )))});
            }
        }
        .key_pair
        .clone();

        Box::new(async_block! {
            info!("Calculating nonce");
            let nonce = await!(web3.eth()
                               .transaction_count(key.address(), None)
            )?;
            info!("Estimating gas price");
            let gas_price = await!(web3.eth().gas_price())?;
            info!("Estimating gas usage");
            let call_request = web3::types::CallRequest {
                from: Some(key.address()),
                to: request.concern.contract_address,
                gas: None,
                gas_price: None,
                value: Some(request.value),
                data: Some(Bytes(request.data.clone())),
            };
            let gas = await!(web3.eth().estimate_gas(call_request, None))
                .chain_err(|| format!("could not estimate gas usage"))?;
            info!("Sending transaction");
            let signed_tx = Transaction {
                action: Action::Call(request.concern.contract_address),
                nonce: nonce,
                // do something better then double
                gas_price: U256::from(2).checked_mul(gas_price).unwrap(),
                // do something better then double
                gas: U256::from(2).checked_mul(gas).unwrap(),
                value: request.value,
                data: request.data,
            }
            .sign(&key.secret(), Some(69));
            let raw = Bytes::from(rlp::encode(&signed_tx));
            let a = await!(web3.eth().send_raw_transaction(raw));
            info!("Transaction sent with hash: {:?}", a?);
            assert_eq!(Address::from(keccak(key.public())), signed_tx.sender());
            assert_eq!(signed_tx.chain_id(), Some(69));
            Ok(())
        })
    }
}
