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
use ethkey::{KeyPair, Secret};
use futures::prelude::{async_block, await};
use keccak_hash::keccak;
use std::collections::HashMap;
use std::rc::Rc;
use web3::futures::Future;
use web3::types::Bytes;

pub enum Strategy {
    Deadline(std::time::Instant),
}

pub struct TransactionRequest {
    pub concern: configuration::Concern,
    pub value: U256,
    pub data: Vec<u8>,
    pub strategy: Strategy,
}

struct ConcernData {
    secret: Secret,
}

pub struct TransactionManager {
    config: Configuration,
    concern_data: HashMap<Concern, ConcernData>,
    web3: Rc<web3::Web3<web3::transports::Http>>,
    _eloop: web3::transports::EventLoopHandle, // kept to stay in scope
}

const KEY: &str =
    "339565dd96968ad4fba67e320bc9cf07808298d3654634e1bcc3b46350964f6e";

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// we need to implement recovering keys for concerns in keystore
fn recover_key(ref _concern: &Concern) -> Result<Secret> {
    let key_string: String = std::env::var("CONCERN_KEY").chain_err(|| {
        format!("for now, keys can only be provided as env variable")
    })?;
    //secret_from_hex_string(key_string)
    //Ok(KeyPair::from_secret_slice(key_string.as_bytes())?.secret())
    Ok(key_string
        .trim_start_matches("0x")
        .parse()
        .chain_err(|| format!("failed to parse key"))?)
}

impl TransactionManager {
    pub fn new(config: Configuration) -> Result<TransactionManager> {
        // Change this by a properly handled ethstore
        // let mut hard_coded_keys = HashMap::new();
        // hard_coded_keys.insert(Address::from(ADDR), Secret::from(KEY));

        let url = config.url.clone();

        let mut concern_data = HashMap::new();
        for concern in config.clone().concerns {
            let key = recover_key(&concern)?;
            concern_data.insert(concern, ConcernData { secret: key });
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
        request: TransactionRequest,
    ) -> Box<Future<Item = (), Error = Error>> {
        // async_block needs owned values, so let us clone
        let web3 = Rc::clone(&self.web3);

        //let key = KeyPair::from_secret(self.secret.clone()).unwrap();
        let key = KeyPair::from_secret(Secret::from(KEY)).unwrap();

        Box::new(async_block! {
            let nonce = await!(web3.eth()
                               .transaction_count(key.address(), None)
            )?;
            let signed_tx = Transaction {
                action: Action::Call(key.address()),
                nonce: nonce,
                gas_price: U256::from(3_000_000),
                gas: U256::from(50_000),
                value: U256::from(1),
                data: b"".to_vec(),
            }
            .sign(&key.secret(), Some(69));

            let raw = Bytes::from(rlp::encode(&signed_tx));

            let a = await!(web3.eth().send_raw_transaction(raw));
            println!("{:?}", a?);
            assert_eq!(Address::from(keccak(key.public())), signed_tx.sender());
            assert_eq!(signed_tx.chain_id(), Some(69));
            Ok(())
        })
    }
}
