pub mod dapp;

extern crate configuration;
extern crate emulator;
extern crate error;
extern crate ethereum_types;
extern crate tokio;
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
use std::collections::HashSet;
use std::time::Duration;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::timer::Interval;
use transaction::{Strategy, TransactionManager, TransactionRequest};
use utils::{print_error, EthWeb3};
use web3::futures::future::lazy;
use web3::futures::sync::mpsc;
use web3::futures::{future, stream, Future, Stream};

//use std::sync::mpsc;
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

    pub fn run<T: DApp<()>>(&self) {
        enum Message {
            Socket(TcpStream),
            Tick,
            Done,
        }

        struct State {
            handled: HashSet<usize>,
        }

        // Interval at which we poll and dispatch instances
        let tick_duration = Duration::from_secs(3);

        let interval = Interval::new_interval(tick_duration)
            .map(|_| Message::Tick)
            .map_err(|_| ())
            .take(10);

        // accept queries from port 3003
        let addr = "127.0.0.1:3003".parse().unwrap();
        let listener =
            TcpListener::bind(&addr).expect("could not bind to port 3003");
        let incoming = listener
            .incoming()
            .map(|socket| Message::Socket(socket))
            .map_err(|_| ());

        // join all the streams
        let messages = interval.select(incoming);

        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        // use this state to avoid treating an instance twice
        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        let initial_state = State {
            handled: HashSet::new(),
        };

        // clone pointers to move inside the process
        let main_concern_process = (&self).config.main_concern.clone();
        let transaction_manager_process = (&self).transaction_manager.clone();
        let state_manager_process = (&self).state_manager.clone();
        let emulator_process = (&self).emulator.clone();
        let current_archive_process = (&self).current_archive.clone();

        let process = messages
            .fold(
                initial_state,
                move |_state,
                      message|
                      -> Box<Future<Item = State, Error = ()> + Send> {
                    match message {
                        Message::Socket(socket) => {
                            Box::new(
                                io::write_all(socket, "hello world")
                                    // Drop the socket
                                    .map(|_| State {
                                        handled: HashSet::new(),
                                    })
                                    .map_err(|e| {
                                        error!("socket error = {:?}", e)
                                    }),
                            )
                        }
                        Message::Tick => {
                            trace!(
                                "Getting indices for {:?}",
                                main_concern_process
                            );

                            let state_manager_indices =
                                state_manager_process.clone();

                            let stream_of_indices = state_manager_indices
                                .lock()
                                .unwrap()
                                .get_indices(main_concern_process.clone())
                                .map_err(|e| {
                                    print_error(&e.chain_err(|| {
                                        format!("could not get issue indices")
                                    }));
                                })
                                .map(|vector_of_indices| {
                                    stream::iter_ok(vector_of_indices)
                                })
                                .flatten_stream();

                            // clone pointers to move inside each index
                            let main_concern_index =
                                main_concern_process.clone();
                            let transaction_manager_index =
                                transaction_manager_process.clone();
                            let state_manager_index =
                                state_manager_process.clone();
                            let emulator_index = emulator_process.clone();
                            let current_archive_index =
                                current_archive_process.clone();

                            Box::new(
                                stream_of_indices
                                    .inspect(|index| {
                                        trace!("Processing index {}", index)
                                    })
                                    .for_each(move |index| {
                                        tokio::spawn(
                                            execute_reaction::<T>(
                                                main_concern_index,
                                                index,
                                                transaction_manager_index
                                                    .clone(),
                                                state_manager_index.clone(),
                                                emulator_index.clone(),
                                                current_archive_index.clone(),
                                            )
                                            .map_err(|e| print_error(&e)),
                                        );

                                        Ok(())
                                    })
                                    .map(|_| State {
                                        handled: HashSet::new(),
                                    }),
                            )
                        },
                        _ => {
                            Box::new(
                                web3::futures::future::ok::<State, ()>(
                                    State {
                                        handled: HashSet::new(),
                                    }
                                )
                            )
                        }
                    }
                },
            )
            .map(|_| ());

        tokio::run(process);
    }
}

fn execute_reaction<T: DApp<()>>(
    main_concern: Concern,
    index: usize,
    transaction_manager_arc: Arc<Mutex<TransactionManager>>,
    state_manager_arc: Arc<Mutex<StateManager>>,
    emulator_arc: Arc<Mutex<EmulatorManager>>,
    current_archive_arc: Arc<Mutex<Archive>>,
) -> Box<Future<Item = (), Error = Error> + Send> {
    let state_manager_clone = state_manager_arc.clone();
    let state_manager_lock = state_manager_clone.lock().unwrap();

    return Box::new(
        state_manager_lock
            .get_instance(main_concern, index)
            .and_then(
            move |instance| -> Box<Future<Item = (), Error = Error> + Send> {
                let transaction_manager =
                    transaction_manager_arc.lock().unwrap();
                let state_manager = state_manager_arc.lock().unwrap();
                let emulator = emulator_arc.lock().unwrap();
                let mut current_archive = current_archive_arc.lock().unwrap();

                let reaction = match T::react(&instance, &current_archive, &())
                    .chain_err(|| format!("could not get dapp reaction"))
                {
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
                        let current_archive_clone = current_archive_arc.clone();
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
) -> Box<Future<Item = (), Error = Error> + Send> {
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
) -> Box<Future<Item = (), Error = Error> + Send> {
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
) -> Box<Future<Item = (), Error = Error> + Send> {
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
