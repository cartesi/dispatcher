extern crate configuration;
extern crate env_logger;
extern crate envy;
extern crate error;

#[macro_use]
extern crate serde_derive;
extern crate structopt;
#[macro_use]
extern crate log;
extern crate ethcore_transaction;
extern crate ethereum_types;
extern crate ethjson;
extern crate ethkey;
extern crate keccak_hash;
extern crate rlp;
extern crate web3;

use configuration::Configuration;
use error::*;
use ethcore_transaction::{Action, SignedTransaction, Transaction};
use ethereum_types::{Address, U256};
use ethkey::{KeyPair, Public, Secret};
use keccak_hash::keccak;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use structopt::StructOpt;
use web3::futures::{Future, IntoFuture};
use web3::types::Bytes;

pub struct TransactionManager {
    config: Configuration,
    keys: HashMap<Address, Secret>,
    secret: Secret,
    web3: web3::Web3<web3::transports::Http>,
    _eloop: web3::transports::EventLoopHandle, // kept to stay in scope
}

const ADDR: &str = "0x2ad38f50f38abc5cbcf175e1962293eecc7936de";

const KEY: &str =
    "339565dd96968ad4fba67e320bc9cf07808298d3654634e1bcc3b46350964f6e";

impl TransactionManager {
    pub fn new(config: Configuration) -> Result<TransactionManager> {
        // Change this by a properly handled ethstore
        let mut hard_coded_keys = HashMap::new();

        hard_coded_keys.insert(Address::from(ADDR), Secret::from(KEY));

        let url = config.url.clone();
        info!("Trying to connect to Eth node at {}", &url[..]);
        let (_eloop, transport) = web3::transports::Http::new(&url[..])
            .chain_err(|| {
                format!("could not connect to Eth node at url: {}", &url[..])
            })?;

        let my_web3 = web3::Web3::new(transport);

        Ok(TransactionManager {
            config: config,
            keys: hard_coded_keys,
            secret: Secret::from(KEY),
            web3: my_web3,
            _eloop: _eloop,
        })
    }

    pub fn send(&self) -> Result<()> {
        let key = KeyPair::from_secret(self.secret.clone()).unwrap();

        let nonce = self
            .web3
            .eth()
            .transaction_count(key.address(), None)
            .wait()?;
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

        let a = self.web3.eth().send_raw_transaction(raw).wait()?;
        println!("{:?}", a);
        assert_eq!(Address::from(keccak(key.public())), signed_tx.sender());
        assert_eq!(signed_tx.chain_id(), Some(69));
        Ok(())
    }
}
