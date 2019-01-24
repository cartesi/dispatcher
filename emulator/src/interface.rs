extern crate configuration;

use self::configuration::Configuration;
use super::error::*;
use std::path::PathBuf;

pub enum Backing {
    Zeros,
    Path(PathBuf),
}

pub type Word = [u8; 8];

pub struct Ram {
    ilength: Word,
    backing: Backing,
}

pub struct Drive {
    istart: Word,
    ilength: Word, // should this be the log_2 of the length?
    backing: Backing,
}

pub enum DriveId {
    FLASH_0,
    FLASH_1,
    FLASH_2,
    FLASH_3,
    FLASH_4,
    FLASH_5,
    FLASH_6,
    FLASH_7,
    RAM,
}

pub struct MachineSpecification {
    argument: String, // command line argument passed to kernel
    ram: Ram,
    flash0: Drive,
    flash1: Drive,
    flash2: Drive,
    flash3: Drive,
    flash4: Drive,
    flash5: Drive,
    flash6: Drive,
    flash7: Drive,
}

pub type Hash = String;

pub struct Proof {
    address: Word,
    depth: u32,
    root: Hash,
    siblings: Vec<Hash>,
    target: Hash,
}

pub enum Operation {
    Read,
    Write,
}

pub struct Access {
    operation: Operation,
    read: Word,
    written: Word,
    proof: Proof,
}

pub type SessionId = String;

pub struct InitRequest {
    session: SessionId,
    machine: MachineSpecification,
}

pub struct RunRequest {
    session: SessionId,
    time: u64,
}

pub struct DriveRequest {
    session: SessionId,
    drive: DriveId,
}

pub struct ReadRequest {
    session: SessionId,
    address: Word,
}

pub struct ReadResult {
    value: Word,
    proof: Proof,
}

pub type StepResult = Vec<Access>;
