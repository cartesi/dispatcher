extern crate configuration;
extern crate error;
extern crate ethereum_types;
extern crate utils;
extern crate web3;

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate ethabi;
extern crate ethcore_transaction;
extern crate hex;
extern crate serde_json;
extern crate state;
extern crate transaction;

use configuration::Configuration;
pub use error::*;
use ethabi::Token;
use ethereum_types::U256;
use serde_json::Value;
use state::StateManager;
use transaction::{Strategy, TransactionManager, TransactionRequest};
use utils::EthWeb3;
use web3::futures::Future;

pub struct Dispatcher {
    config: Configuration,
    web3: web3::api::Web3<web3::transports::http::Http>,
    transaction_manager: TransactionManager,
    state_manager: StateManager,
}

impl Dispatcher {
    pub fn new() -> Result<Dispatcher> {
        info!("Loading configuration file");
        let config = Configuration::new()
            .chain_err(|| format!("could not load configuration"))?;

        info!("Trying to connect to Eth node at {}", &config.url[..]);
        let (_eloop, transport) = web3::transports::Http::new(&config.url[..])
            .chain_err(|| {
                format!("could not connect to Eth node at url: {}", &config.url)
            })?;

        info!("Testing Ethereum node's functionality");
        let web3 = web3::Web3::new(transport);
        web3.test_connection(&config).wait()?;

        info!("Creating transaction manager");
        let transaction_manager =
            TransactionManager::new(config.clone(), web3.clone()).chain_err(
                || format!("could not create transaction manager"),
            )?;

        info!("Creating state manager");
        let state_manager = StateManager::new(config.clone(), web3.clone())?;

        let dispatcher = Dispatcher {
            config: config,
            web3: web3,
            transaction_manager: transaction_manager,
            state_manager: state_manager,
        };

        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        // should change this to get the list and treat each element
        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        let main_concern = dispatcher.config.concerns[0].clone();

        info!("Getting issues for concern: {:?}", main_concern);
        let issues = dispatcher
            .state_manager
            .get_instances(main_concern.clone())
            .wait()
            .chain_err(|| format!("could not get issues"))?;

        println!("{:?}", issues);

        let instance = dispatcher
            .state_manager
            .get_state(main_concern.clone().contract_address, issues[0])
            .wait();

        println!("{:?}", instance);

        return Ok(dispatcher);

        info!("Getting contract's abi from truffle");
        // change this to proper handling of file
        let truffle_dump = include_str!(
            "/home/augusto/contracts/build/contracts/PartitionInstantiator.json"
        );
        let v: Value = serde_json::from_str(truffle_dump)
            .chain_err(|| format!("could not read truffle json file"))?;

        // failed attempt to check contract's code.
        // Should use the data submitted during transaction creation instead
        // use binary_search or binary_search_by provided by Vec
        //
        // info!("Getting contract's code from node");
        // let code = dispatcher
        //     .web3
        //     .eth()
        //     .code(main_concern.contract_address, None)
        //     .wait()?;
        // let bytecode = hex::decode(
        //     String::from(v["bytecode"].as_str().unwrap())
        //         .trim_start_matches("0x"),
        // ).unwrap();

        info!("Encoding function call through abi");
        let abi = ethabi::Contract::load(
            serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
        )
        .chain_err(|| format!("could decode json abi"))?;
        let params = vec![
            Token::Address(main_concern.user_address),
            Token::Address(main_concern.contract_address),
            Token::FixedBytes(vec![b'X'; 32]),
            Token::FixedBytes(vec![b'U'; 32]),
            Token::Uint(U256::from(122 as u64)),
            Token::Uint(U256::from(10)),
            Token::Uint(U256::from(100)),
        ];
        let data = abi
            .function("instantiate".into())
            .and_then(|function| function.encode_input(&params))
            .chain_err(|| format!("could not encode function parameters"))?;

        let req = TransactionRequest {
            concern: main_concern,
            value: U256::from(0),
            data: data,
            strategy: Strategy::Simplest,
        };

        info!("Sending call to instantiate");
        dispatcher
            .transaction_manager
            .send(req)
            .wait()
            .chain_err(|| format!("transaction manager failed to send"))?;

        Ok(dispatcher)
    }
}
