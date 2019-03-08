extern crate futures;

use self::futures::future::{ok, Future};
use configuration::Configuration;
use emulator_interface::manager;
use emulator_interface::manager_grpc::*;
use error::*;
use grpc::{ClientStubExt, RequestOptions};

pub use types::{
    Access, AccessOperation, Proof, SessionRunRequest, SessionRunResult,
    SessionStepRequest, SessionStepResult,
};

pub struct EmulatorManager {
    client: MachineManagerClient,
}

impl EmulatorManager {
    pub fn new(
        config: Configuration //web3: web3::Web3<web3::transports::http::Http>
    ) -> Result<EmulatorManager> {
        let client_conf = Default::default();
        let client: MachineManagerClient = MachineManagerClient::new_plain(
            "::1",
            config.emulator_port,
            client_conf,
        )
        .unwrap();

        Ok(EmulatorManager { client: client })
    }

    // pub fn new_session(&self, _: NewSessionRequest) -> (Hash) {
    //     [
    //         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //         0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //     ]
    //     .into()
    // }
    pub fn run(
        &self,
        request: SessionRunRequest,
    ) -> Box<Future<Item = SessionRunResult, Error = Error> + Send> {
        //"Not implemented!".into()
        let mut req = manager::SessionRunRequest::new();
        req.set_session_id(request.session_id);
        req.set_times(request.times);
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
    // pub fn read(&self, _: ReadRequest) -> (ReadResult) {
    //     ReadResult {
    //         value: [0, 0, 0, 0, 0, 0, 0, 0],
    //         proof: Proof {
    //             address: [0, 0, 0, 0, 0, 0, 0, 0],
    //             depth: 61,
    //             //root: "0x0000".to_string(),
    //             siblings: vec![],
    //             //target: "0x0000".to_string(),
    //         },
    //     }
    // }
    // pub fn provedrive(&self, _: DriveRequest) -> (Proof) {
    //     Proof {
    //         address: [0, 0, 0, 0, 0, 0, 0, 0],
    //         depth: 61,
    //         //root: "0x0000".to_string(),
    //         siblings: vec![],
    //         //target: "0x0000".to_string(),
    //     }
    // }
    // pub fn getbacking(&self, _: DriveRequest) -> (Backing) {
    //     Backing::Zeros
    // }
}
