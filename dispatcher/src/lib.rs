// Dispatcher provides the infrastructure to support the development of DApps,
// mediating the communication between on-chain and off-chain components.

// Copyright (C) 2019 Cartesi Pte. Ltd.

// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.

// This program is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
// PARTICULAR PURPOSE. See the GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// Note: This component currently has dependencies that are licensed under the GNU
// GPL, version 3, and so you should treat this component as a whole as being under
// the GPL version 3. But all Cartesi-written code in this component is licensed
// under the Apache License, version 2, or a compatible permissive license, and can
// be used independently under the Apache v2 license. After this component is
// rewritten, the entire component will be released under the Apache v2 license.

pub mod dapp;

extern crate configuration;
extern crate error;
extern crate ethereum_types;
extern crate grpc;
extern crate tokio;
extern crate utils;
extern crate web3;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate ethabi;
extern crate hex;
extern crate hyper;
extern crate serde;
extern crate serde_json;
extern crate state;
extern crate transaction;
extern crate transport;

use std::str;

use configuration::{Concern, Configuration};
pub use error::*;
use grpc::{Client, RequestOptions};
use hyper::service::service_fn;
use hyper::{Body, Request, Response, Server};
use state::StateManager;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::prelude::Sink;
use tokio::timer::Interval;
use transaction::{TransactionManager, TransactionRequest};
use utils::{print_error, EthWeb3};
use web3::futures::future::lazy;
use web3::futures::sync::{mpsc, oneshot};
use web3::futures::{stream, Future, Stream};
use transport::GenericTransport;

pub use dapp::{
    AddressArray, AddressArray3, AddressField, Archive, BoolArray, BoolField,
    Bytes32Array, Bytes32Array3, Bytes32Field, DApp, FieldType, Reaction, String32Field,
    U256Array, U256Array3, U256Array4, U256Array5, U256Array6, U256Array7, U256Array9,
    U256Field,
};

/// Responsible for querying the state of each concern, get a reaction
/// from the dapp and submit reactions for either the Transaction Manager or
/// the other services (Emulator, Logger, etc)
pub struct Dispatcher {
    config: Configuration,
    _web3: web3::api::Web3<GenericTransport>, // to stay in scope
    _eloop: web3::transports::EventLoopHandle, // kept to stay in scope
    assets: Assets,
}

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// should we put the Arc<Mutex<>> in the Assets instead of in each of them?
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

/// All the assets in the dispatcher that have to be shared by tokio tasks
struct Assets {
    transaction_manager: Arc<Mutex<TransactionManager>>,
    state_manager: Arc<Mutex<StateManager>>,
    archive: Arc<Mutex<Archive>>,
    clients: Arc<Mutex<HashMap<String, Arc<Mutex<Client>>>>>,
}

impl Assets {
    fn clone(&self) -> Self {
        Assets {
            transaction_manager: self.transaction_manager.clone(),
            state_manager: self.state_manager.clone(),
            archive: self.archive.clone(),
            clients: self.clients.clone(),
        }
    }
}

impl Dispatcher {
    /// Creates a new dispatcher loading configuration from file indicated
    /// in either command line or environmental variable
    pub fn new() -> Result<Dispatcher> {
        info!("Loading configuration file");
        let config = Configuration::new()
            .chain_err(|| format!("could not load configuration"))?;

        info!("Trying to connect to Eth node at {}", &config.url[..]);
        let (_eloop, transport) = GenericTransport::new(&config.url[..])
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

        info!("Creating archive");
        let archive = Archive::new()?;

        info!("Creating grpc client");
        let mut clients = HashMap::new();
        for service in config.services.iter() {
            let client = Client::new_plain(
                &service.transport.address.clone(),
                service.transport.port.clone(),
                Default::default(),
            )?;
            clients.insert(service.name.clone(), Arc::new(Mutex::new(client)));
        }

        let dispatcher = Dispatcher {
            config: config,
            _web3: web3,
            _eloop: _eloop,
            assets: Assets {
                transaction_manager: Arc::new(Mutex::new(transaction_manager)),
                state_manager: Arc::new(Mutex::new(state_manager)),
                archive: Arc::new(Mutex::new(archive)),
                clients: Arc::new(Mutex::new(clients)),
            },
        };

        return Ok(dispatcher);
    }

    pub fn run<T: DApp<()>>(&self) {
        // get owned copies of main_concern and assets to move into task
        let main_concern_run = (&self).config.main_concern.clone();
        let assets_run = (&self).assets.clone();
        let port = (&self).config.query_port;
        tokio::run(lazy(move || {
            let (query_tx, query_rx) = mpsc::channel(1_024);

            // spawn the background process that handles all the
            // instances and delegates work to other tokio tasks
            tokio::spawn(background_process::<T>(
                main_concern_run,
                assets_run,
                query_rx,
            ));

            // start listening to port for state queries
            let addr = match std::env::var_os("DOCKER") {
                Some(val) => {
                    if val == "TRUE" {
                        trace!("Binding to 0.0.0.0 as dispatcher running inside a docker");
                        ([0, 0, 0, 0], port).into()
                    } else {
                        ([127, 0, 0, 1], port).into()
                    }
                }
                None => ([127, 0, 0, 1], port).into(),
            };
            let listener = tokio::net::TcpListener::bind(&addr)
                .expect("could not bind to port");

            // to each incomming connection, create a replier that
            // knows how to handle queries about the state of each
            // instance
            Server::builder(listener.incoming())
                .serve(move || {
                    let tx = query_tx.clone();
                    service_fn(move |req| replier(tx.clone(), req))
                })
                .map_err(|e| error!("error in socket {}", e))
        }))
    }
}

/// The query handle comes with a query and a oneshot communication
/// channel for sending the result
#[derive(Debug)]
struct QueryHandle {
    query: Query,
    oneshot: web3::futures::sync::oneshot::Sender<String>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct PostBody {
    index: usize,
    payload: String,
}

/// All possible queries that can be done to the server concerning the
/// state of instances
#[derive(Debug, PartialEq, Deserialize)]
enum Query {
    Indices,
    Instance(usize),
    Post(PostBody),
}

// creates a future representing the background process that organizes
// all instances and delegates tasks
fn background_process<T: DApp<()>>(
    main_concern: Concern,
    assets: Assets,
    query_rx: mpsc::Receiver<QueryHandle>,
) -> Box<dyn Future<Item = (), Error = ()> + Send> {
    // during the course of execution, there are periodic (Tick) events,
    // or external queries concerning the current state. we need to react
    // to these two types of messages (inspired by Elm programming language)
    enum Message {
        Tick,
        Asked(QueryHandle),
    }

    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    // use this state to avoid treating an instance twice
    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    struct State {
        _handled: HashSet<usize>,
    }

    // Interval at which we poll and dispatch instances
    let tick_duration = Duration::from_secs(6);
    let interval = Interval::new_interval(tick_duration)
        .map(|_| Message::Tick)
        .map_err(|_| ());

    let messages = query_rx
        .map(Message::Asked)
        // Merge queries received from channel to the stream of Ticks
        .select(interval);

    // Initialize state as empty
    let initial_state = State {
        _handled: HashSet::new(),
    };

    // clone assets to move them inside the closure
    let main_concern_fold = main_concern.clone();
    let assets_fold = assets.clone();

    return Box::new(
        messages
            .fold(
                initial_state,
                move |_state,
                      message|
                      -> Box<dyn Future<Item = State, Error = ()> + Send> {
                    match message {
                        // message is a query, answer it appropriately
                        Message::Asked(q) => {
                            info!("Received query: {:?}", q.query);
                            let state_manager_query = assets_fold
                                .state_manager
                                .clone();
                            match q.query {
                                Query::Indices => {
                                    let indices = state_manager_query
                                        .lock()
                                        .unwrap()
                                        .get_indices(main_concern_fold.clone())
                                        .wait()
                                        .unwrap();
                                    // send result back from oneshot channel
                                    q.oneshot.send(
                                        serde_json::to_string(&indices).unwrap()
                                    ).unwrap();
                                },
                                Query::Instance(i) => {
                                    let instance = state_manager_query
                                        .lock()
                                        .unwrap()
                                        .get_instance(
                                            main_concern_fold.clone(),
                                            i
                                        )
                                        .wait()
                                        .unwrap();
                                    let archive = assets_fold.archive.lock().unwrap();
                                    let pretty_instance = T::get_pretty_instance(&instance, &archive, &()).unwrap();
                                    // send result back from oneshot channel
                                    q.oneshot.send(
                                        serde_json::to_string(&pretty_instance).unwrap()
                                    ).unwrap();
                                },
                                Query::Post(body) => {
                                    // clone assets to move inside
                                    let main_concern_index = main_concern_fold.clone();
                                    let assets_index = assets_fold.clone();

                                    tokio::spawn(
                                        execute_reaction::<T>(
                                            main_concern_index,
                                            body.index,
                                            Some(body.payload),
                                            assets_index.clone(),
                                        )
                                        .map_err(|e| print_error(&e)),
                                    );
                                    let mut ok_status = HashMap::new();
                                    ok_status.insert("status".to_string(), "ok".to_string());
                                    q.oneshot.send(
                                        serde_json::to_string(&ok_status).unwrap()
                                    ).unwrap();
                                }
                            };

                            // for now we don't keep track of the state
                            Box::new(web3::futures::future::ok::<State, ()>(
                                State {
                                    _handled: HashSet::new(),
                                },
                            ))
                        },
                        // received a periodic Tick. We need to check
                        // for new instances and launch tasks for each.
                        Message::Tick => {
                            // clone assets to have static lifetime
                            let state_manager_indices =
                                assets_fold.state_manager.clone();

                            trace!(
                                "Getting indices for {:?}",
                                main_concern_fold
                            );
                            let stream_of_indices = state_manager_indices
                                .lock()
                                .unwrap()
                                .get_indices(main_concern_fold.clone())
                                .map_err(|e| {
                                    print_error(&e.chain_err(|| {
                                        format!("could not get issue indices")
                                    }));
                                })
                                .map(|vector_of_indices| {
                                    stream::iter_ok(vector_of_indices)
                                })
                                .flatten_stream();

                            // clone assets to move inside each index
                            let main_concern_index = main_concern_fold.clone();
                            let assets_index = assets_fold.clone();

                            let returned_state = stream_of_indices
                                .inspect(|index| {
                                    trace!("Processing index {}", index)
                                })
                                .for_each(move |index| {
                                    tokio::spawn(
                                        execute_reaction::<T>(
                                            main_concern_index,
                                            index,
                                            None,
                                            assets_index.clone(),
                                        )
                                        .map_err(|e| print_error(&e)),
                                    );
                                    Ok(())
                                })
                                .map(|_| State {
                                    _handled: HashSet::new(),
                                });
                            Box::new(returned_state)
                        }
                    }
                },
            )
            .map(|_| ()),
    );
}

fn execute_reaction<T: DApp<()>>(
    main_concern: Concern,
    index: usize,
    post_action: Option<String>,
    assets: Assets,
) -> Box<dyn Future<Item = (), Error = Error> + Send> {
    let state_manager_clone = assets.state_manager.clone();
    let state_manager_lock = state_manager_clone.lock().unwrap();

    return Box::new(
        state_manager_lock
            .get_instance(main_concern, index)
            .and_then(
            move |instance| -> Box<dyn Future<Item = (), Error = Error> + Send> {
                let transaction_manager =
                    assets.transaction_manager.lock().unwrap();
                let mut archive = assets.archive.lock().unwrap();
                let clients = assets.clients.lock().unwrap();

                // get reaction from dapp to this instance
                let reaction = match T::react(&instance, &archive, &post_action, &())
                // TODO: may need to uncomment below line
                //    .chain_err(|| format!("could not get dapp reaction"))
                {
                    Ok(r) => r,
                    Err(e) => {
                        match e.kind() {
                            // can't find specific data with `key` in the archive,
                            // try to get it from the service through grpc request
                            ErrorKind::ArchiveMissError(service, key, method, request) => {
                                trace!("handling ArchiveMissError for service: {}, and key: {}", service, key);
                                if let Some(client) = clients.get(&service.clone()) {

                                    let response = grpc_call_unary(
                                        client.clone(),
                                        request.clone(),
                                        method.clone())
                                    .wait_drop_metadata();

                                    match response {
                                        Ok(resp) => {
                                            archive.insert(key.clone(), Ok(resp));
                                            return Box::new(web3::futures::future::ok::<(), _>(()));
                                        }
                                        Err(e) => {
                                            match e {
                                                grpc::Error::GrpcMessage(msg) => {
                                                    archive.insert(key.clone(), Err(msg.grpc_message.clone()));
                                                    return Box::new(web3::futures::future::ok::<(), _>(()));
                                                }
                                                _ => {
                                                    return Box::new(web3::futures::future::err(e.into()));
                                                }
                                            }
                                        }
                                    }
                                }
                                return Box::new(
                                    web3::futures::future::err(
                                        Error::from(format!("Fail to get grpc client of {} service", service))
                                    )
                                );
                            },
                            // the archive consists invalid data for `key`,
                            // remove the entry and let `ArchiveMissError` handle the rest
                            ErrorKind::ArchiveInvalidError(service, key, _m) => {
                                trace!("handling ArchiveInvalidError for service: {}, and key: {}", service, key);
                                archive.remove(key.clone());
                                return Box::new(web3::futures::future::ok::<(), _>(()));
                            },
                            ErrorKind::ArchiveNeedsDummy(service, key, _m) => {
                                trace!("handling ArchiveNeedsDummy for service: {}, and key: {}", service, key);
                                archive.insert(key.clone(), Ok(Vec::new()));
                                return Box::new(web3::futures::future::ok::<(), _>(()));
                            }
                            _ => {
                                return Box::new(web3::futures::future::err(e));
                            }
                        }
                    }
                };
                trace!(
                    "Reaction to instance {} of {} is: {:?}",
                    index,
                    main_concern.contract_address,
                    reaction
                );

                // act according to dapp reaction
                match reaction {
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

fn process_transaction_request(
    main_concern: Concern,
    index: usize,
    transaction_request: TransactionRequest,
    transaction_manager: &TransactionManager,
) -> Box<dyn Future<Item = (), Error = Error> + Send> {
    info!(
        "Send transaction (concern {:?}, index {}): {:?}",
        main_concern, index, transaction_request
    );
    let main_concern_clone = main_concern.clone();
    let index_clone = index.clone();
    let transaction_request_clone = transaction_request.clone();
    Box::new(
        transaction_manager
            .send(transaction_request)
            .map_err(move |e| {
                e.chain_err(move || {
                    format!(
                        "could not send transaction: {:?}, index {}, {:?}",
                        main_concern_clone,
                        index_clone,
                        transaction_request_clone
                    )
                })
            }),
    )
}

// a replier is a tokio task that passes queries about the state of the
// blockchain to the background task. We spawn one for each incomming
// connnection.
fn replier(
    tx: mpsc::Sender<QueryHandle>,
    req: Request<Body>,
) -> Box<dyn Future<Item = Response<Body>, Error = std::io::Error> + Send> {
    let (resp_tx, resp_rx) = oneshot::channel();
    let (_, body) = req.into_parts();

    let body_future = body
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("request error {}", e),
            )
        })
        .concat2();
    let query_future = body_future
        .and_then(|body| {
            let query: Query = match serde_json::from_slice(&body) {
                Ok(q) => q,
                Err(e) => {
                    warn!("could not parse query: {:?}, error {:?}", &body, e);
                    Query::Indices
                }
            };
            // send to background task: the query and the tx for oneshot answer
            tx.send(QueryHandle {
                query: query,
                oneshot: resp_tx,
            })
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("request error {}", e),
                )
            })
            .and_then(|_| {
                // received response from background task
                resp_rx
                    .and_then(|answer| {
                        let response = Response::builder()
                            .header("Content-Type", " application/json")
                            .body(Body::from(answer))
                            .unwrap();
                        Ok(response)
                    })
                    .map_err(|e| {
                        std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("request error {}", e),
                        )
                    })
            })
        })
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("request error {}", e),
            )
        });
    Box::new(query_future)
}

// send grpc request with binary data
fn grpc_call_unary(
    client_arc: Arc<Mutex<Client>>,
    req: Vec<u8>,
    method_name: String,
) -> grpc::SingleResponse<Vec<u8>> {
    let client = client_arc.lock().unwrap();

    let method = Arc::new(grpc::rt::MethodDescriptor {
        name: method_name,
        streaming: grpc::rt::GrpcStreaming::Unary,
        req_marshaller: Box::new(grpc::for_test::MarshallerBytes),
        resp_marshaller: Box::new(grpc::for_test::MarshallerBytes),
    });

    client.call_unary(RequestOptions::new(), req, method)
}
