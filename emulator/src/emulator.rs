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



//! Grpc interface to a machine

extern crate futures;
extern crate configuration;

use configuration::{TransPort};
use self::futures::future::{err, ok, Future};
use emulator_interface::manager_high;
use emulator_interface::manager_high_grpc::*;
use error::*;
use emulator_interface::cartesi_base::*;

use grpc::{ClientStubExt, RequestOptions};

pub use types::{
    Access, AccessOperation, Proof, SessionRunRequest, SessionRunResult,
    SessionStepRequest, SessionStepResult, NewSessionRequest,
};

/// This is an interface that can query (via grpc) a machine server
pub struct EmulatorManager {
    client: MachineManagerHighClient,
}

impl EmulatorManager {
    /// Creates a new emulator manager communicating to a certain port
    pub fn new(emulator_transport: TransPort) -> Result<EmulatorManager> {
        let client_conf = Default::default();
        let client: MachineManagerHighClient =
            MachineManagerHighClient::new_plain(&emulator_transport.address, emulator_transport.port, client_conf)
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

    /// TODO: Fill in the funtion description
    pub fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> Box<Future<Item = Hash, Error = Error> + Send> {
        let mut req = manager_high::NewSessionRequest::new();
        req.set_session_id(request.session_id);
        req.set_machine(request.machine);
        return Box::new(ok((&self)
            .client
            .new_session(RequestOptions::new(), req)
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
