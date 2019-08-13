// Note: This component currently has dependencies that are licensed under the GNU GPL, version 3, and so you should treat this component as a whole as being under the GPL version 3. But all Cartesi-written code in this component is licensed under the Apache License, version 2, or a compatible permissive license, and can be used independently under the Apache v2 license. After this component is rewritten, the entire component will be released under the Apache v2 license.

// Copyright 2019 Cartesi Pte. Ltd.

// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.




//! Grpc interface to a machine

extern crate futures;

use self::futures::future::{err, ok, Future};
use emulator_interface::manager_high;
use emulator_interface::manager_high_grpc::*;
use error::*;

use grpc::{ClientStubExt, RequestOptions};

pub use types::{
    Access, AccessOperation, Proof, SessionRunRequest, SessionRunResult,
    SessionStepRequest, SessionStepResult,
};

/// This is an interface that can query (via grpc) a machine server
pub struct EmulatorManager {
    client: MachineManagerHighClient,
}

impl EmulatorManager {
    /// Creates a new emulator manager communicating to a certain port
    pub fn new(port: u16) -> Result<EmulatorManager> {
        let client_conf = Default::default();
        let client: MachineManagerHighClient =
            MachineManagerHighClient::new_plain("127.0.0.1", port, client_conf)
                .unwrap();
        //ClientBuilder::new("127.0.0.1", port)
        //    .conf(client_conf)
        //    .build()
        //    .map(|c| Self::with_client(Arc::new(c)))
        //    .unwrap();
        Ok(EmulatorManager { client: client })
    }

    /// Runs the machine until certain times, returning the corresponding hashes
    pub fn run(
        &self,
        request: SessionRunRequest,
    ) -> Box<Future<Item = SessionRunResult, Error = Error> + Send> {
        let mut req = manager_high::SessionRunRequest::new();
        req.set_session_id(request.session_id);
        req.set_final_cycles(request.times);
        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        // Fix the mess below, but it is mainly a fault of rust's grpc
        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        let grpc_request = (&self)
            .client
            .session_run(RequestOptions::new(), req)
            .0
            .wait();

        match grpc_request {
            Ok(response) => {
                return Box::new(ok(response
                    .1
                    .wait()
                    .expect("Problem with gprc second future")
                    .0
                    .into()));
            }
            Err(e) => {
                println!("{:?}", e);
                return Box::new(err(e.into()));
            }
        }
    }

    /// Runs one step of the machine (at specified time), returning the
    /// log of all accesses to the memory (reads or writes).
    pub fn step(
        &self,
        request: SessionStepRequest,
    ) -> Box<Future<Item = SessionStepResult, Error = Error> + Send> {
        let mut req = manager_high::SessionStepRequest::new();
        req.set_session_id(request.session_id);
        req.set_initial_cycle(request.time);
        return Box::new(ok((&self)
            .client
            .session_step(RequestOptions::new(), req)
            .0
            .wait()
            .unwrap()
            .1
            .wait()
            .unwrap()
            .0
            .into()));
    }
}
