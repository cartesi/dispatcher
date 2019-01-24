mod interface;

extern crate configuration;
extern crate env_logger;
extern crate error;

#[macro_use]
extern crate log;

use configuration::Configuration;
use error::*;
use interface::{
    Access, Backing, Drive, DriveId, DriveRequest, Hash, InitRequest,
    MachineSpecification, Operation, Proof, Ram, ReadRequest, ReadResult,
    RunRequest, SessionId, StepResult, Word,
};

pub struct EmulatorManager {
    config: Configuration,
}

impl EmulatorManager {
    pub fn new(
        config: Configuration //web3: web3::Web3<web3::transports::http::Http>
    ) -> Result<EmulatorManager> {
        Ok(EmulatorManager { config: config })
    }

    pub fn init(_: InitRequest) -> (Hash) {
        "Not implemented!".into()
    }
    pub fn run(_: RunRequest) -> (Hash) {
        "Not implemented!".into()
    }
    pub fn snapshot(_: SessionId) {}
    pub fn step(_: SessionId) -> (StepResult) {}
    pub fn read(_: ReadRequest) -> (ReadResult) {}
    pub fn provedrive(_: DriveRequest) -> (Proof) {}
    pub fn getbacking(_: DriveRequest) -> (Backing) {}
}
