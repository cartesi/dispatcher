//! Grpc interface to a machine

extern crate futures;

use self::futures::future::{ok, Future};
use emulator_interface::manager;
use emulator_interface::manager_grpc::*;
use error::*;
use grpc::{ClientStubExt, RequestOptions};

pub use types::{
    Access, AccessOperation, Proof, SessionRunRequest, SessionRunResult,
    SessionStepRequest, SessionStepResult,
};

/// This is an interface that can query (via grpc) a machine server
pub struct EmulatorManager {
    client: MachineManagerClient,
}

impl EmulatorManager {
    /// Creates a new emulator manager communicating to a certain port
    pub fn new(port: u16) -> Result<EmulatorManager> {
        let client_conf = Default::default();
        let client: MachineManagerClient =
            MachineManagerClient::new_plain("::1", port, client_conf).unwrap();
        Ok(EmulatorManager { client: client })
    }

    /// Runs the machine until certain times, returning the corresponding hashes
    pub fn run(
        &self,
        request: SessionRunRequest,
    ) -> Box<Future<Item = SessionRunResult, Error = Error> + Send> {
        let mut req = manager::SessionRunRequest::new();
        req.set_session_id(request.session_id);
        req.set_times(request.times);
        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        // Fix the mess below, but it is mainly a fault of rust's grpc
        // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
        return Box::new(ok((&self)
            .client
            .session_run(RequestOptions::new(), req)
            .0
            .wait()
            .unwrap()
            .1
            .wait()
            .unwrap()
            .0
            .into()));
    }

    /// Runs one step of the machine (at specified time), returning the
    /// log of all accesses to the memory (reads or writes).
    pub fn step(
        &self,
        request: SessionStepRequest,
    ) -> Box<Future<Item = SessionStepResult, Error = Error> + Send> {
        let mut req = manager::SessionStepRequest::new();
        req.set_session_id(request.session_id);
        req.set_time(request.time);
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
