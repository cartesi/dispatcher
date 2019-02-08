#![feature(proc_macro_hygiene, generators)]

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

use configuration::{Concern, Configuration};
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

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

pub use dapp::{
    AddressField, Archive, BoolArray, Bytes32Array, Bytes32Field, DApp,
    FieldType, Reaction, SamplePair, SampleRequest, SampleRun, SampleStep,
    String32Field, U256Array, U256Array5, U256Field,
};

pub struct Dispatcher {
    config: Configuration,
    web3: web3::api::Web3<web3::transports::http::Http>,
    _eloop: web3::transports::EventLoopHandle, // kept to stay in scope
    transaction_manager: Arc<Mutex<TransactionManager>>,
    state_manager: Arc<Mutex<StateManager>>,
    emulator: Arc<Mutex<EmulatorManager>>,
    current_archive: Arc<Mutex<Archive>>,
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

        info!("Creating emulator client");
        let emulator = EmulatorManager::new(config.clone())?;

        info!("Creating archive");
        let mut current_archive = Archive::new();

        let dispatcher = Dispatcher {
            config: config,
            web3: web3,
            _eloop: _eloop,
            transaction_manager: Arc::new(Mutex::new(transaction_manager)),
            state_manager: Arc::new(Mutex::new(state_manager)),
            emulator: Arc::new(Mutex::new(emulator)),
            current_archive: Arc::new(Mutex::new(current_archive)),
        };

        return Ok(dispatcher);
    }

    pub fn run<T: DApp<()>>(&self) -> Result<()> {
        let main_concern = (&self).config.main_concern.clone();

        trace!("Getting instances for {:?}", main_concern);

        let indices: Vec<usize>;
        {
            let s = &self.state_manager.clone();
            indices = s
                .lock()
                .unwrap()
                .get_indices(main_concern.clone())
                .wait()
                .chain_err(|| format!("could not get issues"))?;
        }

        let mut thread_handles = vec![];

        for index in indices.into_iter() {
            // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            // temporary for testing purposes
            // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            // if *instance != 13 as usize {
            //     continue;
            // }

            //            let (tx, rx) = mpsc::channel();

            let transaction_manager_clone = (&self).transaction_manager.clone();
            let state_manager_clone = (&self).state_manager.clone();
            let emulator_clone = (&self).emulator.clone();
            let current_archive_clone = (&self).current_archive.clone();

            let mut executer = move || -> Result<()> {
                execute_reaction::<T>(
                    main_concern,
                    index,
                    transaction_manager_clone,
                    state_manager_clone,
                    emulator_clone,
                    current_archive_clone,
                )
                .wait()
            };

            thread_handles.push(thread::spawn(executer));
        }

        for t in thread_handles {
            t.join().unwrap();
        }
        return Ok(());
    }
}

fn execute_reaction<T: DApp<()>>(
    main_concern: Concern,
    index: usize,
    transaction_manager_arc: Arc<Mutex<TransactionManager>>,
    state_manager_arc: Arc<Mutex<StateManager>>,
    emulator_arc: Arc<Mutex<EmulatorManager>>,
    current_archive_arc: Arc<Mutex<Archive>>,
) -> Box<Future<Item = (), Error = Error>> {
    let state_manager_clone = state_manager_arc.clone();
    let state_manager_lock = state_manager_clone.lock().unwrap();

    return Box::new(
        state_manager_lock
            .get_instance(main_concern, index)
            .and_then(
                move |instance| -> Box<Future<Item = (), Error = Error>> {
                    let transaction_manager =
                        transaction_manager_arc.lock().unwrap();
                    let state_manager = state_manager_arc.lock().unwrap();
                    let emulator = emulator_arc.lock().unwrap();
                    let mut current_archive =
                        current_archive_arc.lock().unwrap();

                    let reaction =
                        match T::react(&instance, &current_archive, &())
                            .chain_err(|| {
                                format!("could not get dapp reaction")
                            }) {
                            Ok(r) => r,
                            Err(e) => {
                                return Box::new(web3::futures::future::err(e));
                            }
                        };
                    trace!(
                        "Reaction to instance {} of {} is: {:?}",
                        index,
                        main_concern.contract_address,
                        reaction
                    );

                    match reaction {
                        Reaction::Request(run_request) => {
                            let current_archive_clone =
                                current_archive_arc.clone();
                            process_run_request(
                                main_concern,
                                index,
                                run_request,
                                &emulator,
                                current_archive_clone,
                            )
                        }
                        Reaction::Step(step_request) => process_step_request(
                            main_concern,
                            index,
                            step_request,
                            &emulator,
                            &mut current_archive,
                        ),
                        Reaction::Transaction(transaction_request) => {
                            process_transaction_request(
                                main_concern,
                                index,
                                transaction_request,
                                &transaction_manager,
                            )
                        }
                        Reaction::Idle => {
                            Box::new(web3::futures::future::ok::<(), _>(()))
                        }
                    }
                },
            ),
    );
}

fn process_run_request(
    main_concern: Concern,
    index: usize,
    run_request: SampleRequest,
    emulator: &EmulatorManager,
    current_archive_arc: Arc<Mutex<Archive>>,
) -> Box<Future<Item = (), Error = Error>> {
    return Box::new(emulator
        .run(RunRequest {
            session: run_request.id.clone(),
            times: run_request.times.clone().iter().map(U256::as_u64).collect(),
        })
        .0
        .map_err(|e| {
            Error::from(ErrorKind::GrpcError(format!(
                "could not run emulator: {}",
                e
            )))
        })
        .then(move |grpc_result| {
            // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            // implement proper error and metadata
            // handling
            // but what on earth is going on with grpc?
            // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            let resulting_hash = grpc_result.unwrap().1.wait().unwrap().0;
            info!(
                "Run machine. Concern {:?}, index {}, request {:?}, answer: {:?}",
                main_concern, index, run_request, resulting_hash
            );
            let store_result_in_archive: Result<Vec<()>> = run_request
                .times
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
                            let mut current_archive =
                                current_archive_arc.lock().unwrap();
                            add_run(
                                &mut current_archive,
                                run_request.id.clone(),
                                time,
                                sent_hash,
                            );
                            Ok(())
                        }
                        Err(e) => Err(e.into()),
                    }
                })
                .collect();
            match store_result_in_archive.chain_err(|| {
                format!(
                    "could not convert to hash one of these: {:?}",
                    resulting_hash.hashes
                )
            }) {
                Ok(r) => return web3::futures::future::ok::<(), _>(()),
                Err(e) => {
                    return web3::futures::future::err(e);
                }
            };
        }));
}

fn process_step_request(
    main_concern: Concern,
    index: usize,
    step_request: dapp::StepRequest,
    emulator: &EmulatorManager,
    current_archive: &Archive,
) -> Box<Future<Item = (), Error = Error>> {
    info!(
        "Step request. Concern {:?}, index {}, request {:?}",
        main_concern,
        index,
        emulator.step(StepRequest {
            session: step_request.id.clone(),
            time: step_request.time.as_u64(),
        })
    );

    return Box::new(web3::futures::future::ok::<(), Error>(()));
}

fn process_transaction_request(
    main_concern: Concern,
    index: usize,
    transaction_request: TransactionRequest,
    transaction_manager: &TransactionManager,
) -> Box<Future<Item = (), Error = Error>> {
    info!(
        "Send transaction (concern {:?}, index {}): {:?}",
        main_concern, index, transaction_request
    );
    Box::new(transaction_manager.send(transaction_request).map_err(|e| {
        e.chain_err(|| format!("transaction manager failed to send"))
    }))
}

pub fn add_run(archive: &mut Archive, id: String, time: U256, hash: H256) {
    let mut samples = archive.entry(id.clone()).or_insert(SamplePair {
        run: SampleRun::new(),
        step: SampleStep::new(),
    });
    //samples.0.insert(time, hash);
    if let Some(s) = samples.run.insert(time, hash) {
        warn!("Machine {} at time {} recomputed", id, time);
    }
}

pub fn add_step(
    archive: &mut Archive,
    id: String,
    time: U256,
    proof: dapp::Proof,
) {
    let mut samples = archive.entry(id.clone()).or_insert(SamplePair {
        run: SampleRun::new(),
        step: SampleStep::new(),
    });
    //samples.0.insert(time, hash);
    if let Some(s) = samples.step.insert(time, proof) {
        warn!("Machine {} at time {} recomputed", id, time);
    }
}
