pub mod dapp;

extern crate configuration;
extern crate emulator;
extern crate error;
extern crate ethereum_types;
extern crate utils;
extern crate web3;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate ethabi;
extern crate ethcore_transaction;
extern crate hex;
extern crate serde;
extern crate serde_json;
extern crate state;
extern crate transaction;

use configuration::Configuration;
use emulator::EmulatorManager;
use emulator::{
    Access, Backing, Drive, DriveId, DriveRequest, Hash, InitRequest,
    MachineSpecification, Operation, Proof, Ram, ReadRequest, ReadResult,
    RunRequest, SessionId, StepRequest, StepResult, Word,
};
pub use error::*;
use ethabi::Token;
use ethereum_types::{H256, U256};
use serde_json::Value;
use state::StateManager;
use transaction::{Strategy, TransactionManager, TransactionRequest};
use utils::EthWeb3;
use web3::futures::Future;

pub use dapp::{
    add_run, add_step, AddressField, Archive, BoolArray, Bytes32Array,
    Bytes32Field, DApp, FieldType, Reaction, SampleRequest, String32Field,
    U256Array, U256Array5, U256Field,
};

pub struct Dispatcher {
    config: Configuration,
    web3: web3::api::Web3<web3::transports::http::Http>,
    _eloop: web3::transports::EventLoopHandle, // kept to stay in scope
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
        let state_manager = StateManager::new(config.clone())?;

        let dispatcher = Dispatcher {
            config: config,
            web3: web3,
            _eloop: _eloop,
            transaction_manager: transaction_manager,
            state_manager: state_manager,
        };

        return Ok(dispatcher);
    }

    pub fn run<T: DApp<()>>(&self) -> Result<()> {
        let emulator = EmulatorManager::new((&self).config.clone())?;

        let main_concern = (&self).config.main_concern.clone();

        let mut current_archive = Archive::new();

        for _ in 0..4 {
            trace!("Getting instances for {:?}", main_concern);
            let instances = &self
                .state_manager
                .get_instances(main_concern.clone())
                .wait()
                .chain_err(|| format!("could not get issues"))?;

            for instance in instances.iter() {
                // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                // temporary for testing purposes
                // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                if *instance != 13 as usize {
                    //continue;
                }
                let i = &self
                    .state_manager
                    .get_instance(main_concern, *instance)
                    .wait()?;

                let reaction = T::react(i, &current_archive, &())
                    .chain_err(|| format!("could not get dapp reaction"))?;
                trace!(
                    "Reaction to instance {} of {} is: {:?}",
                    instance,
                    main_concern.contract_address,
                    reaction
                );

                match reaction {
                    Reaction::Request(run_request) => {
                        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                        // implement proper error and metadata
                        // handling
                        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                        let resulting_hash = emulator
                            .run(RunRequest {
                                session: run_request.0.clone(),
                                times: run_request
                                    .1
                                    .clone()
                                    .iter()
                                    .map(U256::as_u64)
                                    .collect(),
                            })
                            .wait()
                            .chain_err(|| {
                                format!("could not contact emulator")
                            })?
                            .1;
                        info!(
                            "Run machine, request {:?}, answer: {:?}",
                            run_request, resulting_hash
                        );
                        let result_or_error: Result<Vec<()>> = run_request
                            .1
                            .clone()
                            .into_iter()
                            .zip(resulting_hash.hashes.clone().iter())
                            .map(|(time, hash)| -> Result<()> {
                                match hash
                                    .hash
                                    .clone()
                                    .trim_start_matches("0x")
                                    .parse::<H256>()
                                {
                                    Ok(sent_hash) => {
                                        add_run(
                                            &mut current_archive,
                                            run_request.0.clone(),
                                            time,
                                            sent_hash,
                                        );
                                        Ok(())
                                    }
                                    Err(e) => Err(e.into()),
                                }
                            })
                            .collect();
                        result_or_error.chain_err(|| {
                            format!(
                                "could not convert to hash one of these: {:?}",
                                resulting_hash.hashes
                            )
                        })?;
                    }
                    Reaction::Step(step_request) => {
                        info!(
                            "Step request: {:?}",
                            emulator.step(StepRequest {
                                session: step_request.0,
                                time: step_request.1.as_u64(),
                            })
                        );
                    }
                    Reaction::Transaction(transaction_request) => {
                        info!(
                            "Send transaction (concern {:?}, index {}): {:?}",
                            main_concern, instance, transaction_request
                        );
                        &self
                            .transaction_manager
                            .send(transaction_request)
                            .wait()
                            .chain_err(|| {
                                format!("transaction manager failed to send")
                            })?;
                    }
                    Reaction::Idle => {}
                }
            }
        }

        return Ok(());

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

        let req = TransactionRequest {
            concern: main_concern,
            function: "instantiate".into(),
            value: U256::from(0),
            data: params,
            strategy: Strategy::Simplest,
        };

        info!("Sending call to instantiate");
        &self
            .transaction_manager
            .send(req)
            .wait()
            .chain_err(|| format!("transaction manager failed to send"))?;
    }
}
