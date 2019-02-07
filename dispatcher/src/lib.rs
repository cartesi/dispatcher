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
    FieldType, Reaction, SampleRequest, SampleRun, SampleStep, String32Field,
    U256Array, U256Array5, U256Field,
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

        let instances: Vec<usize>;
        {
            let s = &self.state_manager.clone();
            instances = s
                .lock()
                .unwrap()
                .get_instances(main_concern.clone())
                .wait()
                .chain_err(|| format!("could not get issues"))?;
        }

        let mut thread_handles = vec![];

        for instance in instances.into_iter() {
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

            let mut executer = move || {
                let t_m_lock = transaction_manager_clone.lock().unwrap();
                let s_m_lock = state_manager_clone.lock().unwrap();
                let e_lock = emulator_clone.lock().unwrap();
                let mut c_a_lock = current_archive_clone.lock().unwrap();

                execute_reaction::<T>(
                    main_concern,
                    &instance,
                    &t_m_lock,
                    &s_m_lock,
                    &e_lock,
                    &mut c_a_lock,
                );
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
    instance: &usize,
    transaction_manager: &TransactionManager,
    state_manager: &StateManager,
    emulator: &EmulatorManager,
    current_archive: &mut Archive,
) -> Result<()> {
    let i = state_manager.get_instance(main_concern, *instance).wait()?;

    let reaction = T::react(&i, current_archive, &())
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
                .chain_err(|| format!("could not contact emulator"))?
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
                                current_archive,
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

            transaction_manager
                .send(transaction_request)
                .wait()
                .chain_err(|| format!("transaction manager failed to send"))?;
        }
        Reaction::Idle => {}
    };
    Ok(())
}

pub fn add_run(archive: &mut Archive, id: String, time: U256, hash: H256) {
    let mut samples = archive
        .entry(id.clone())
        .or_insert((SampleRun::new(), SampleStep::new()));
    //samples.0.insert(time, hash);
    if let Some(s) = samples.0.insert(time, hash) {
        warn!("Machine {} at time {} recomputed", id, time);
    }
}

pub fn add_step(
    archive: &mut Archive,
    id: String,
    time: U256,
    proof: dapp::Proof,
) {
    let mut samples = archive
        .entry(id.clone())
        .or_insert((SampleRun::new(), SampleStep::new()));
    //samples.0.insert(time, hash);
    if let Some(s) = samples.1.insert(time, proof) {
        warn!("Machine {} at time {} recomputed", id, time);
    }
}
