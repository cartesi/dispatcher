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
    pub ilength: Word,
    pub backing: Backing,
}

pub struct Drive {
    pub istart: Word,
    pub ilength: Word, // should this be the log_2 of the length?
    pub backing: Backing,
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
    pub argument: String, // command line argument passed to kernel
    pub ram: Ram,
    pub flash0: Drive,
    pub flash1: Drive,
    pub flash2: Drive,
    pub flash3: Drive,
    pub flash4: Drive,
    pub flash5: Drive,
    pub flash6: Drive,
    pub flash7: Drive,
}

pub type Hash = String;

pub struct Proof {
    pub address: Word,
    pub depth: u32,
    pub root: Hash,
    pub siblings: Vec<Hash>,
    pub target: Hash,
}

pub enum Operation {
    Read,
    Write,
}

pub struct Access {
    pub operation: Operation,
    pub read: Word,
    pub written: Word,
    pub proof: Proof,
}

pub type SessionId = String;

pub struct InitRequest {
    pub session: SessionId,
    pub machine: MachineSpecification,
}

pub struct RunRequest {
    pub session: SessionId,
    pub time: u64,
}

pub struct DriveRequest {
    pub session: SessionId,
    pub drive: DriveId,
}

pub struct ReadRequest {
    pub session: SessionId,
    pub address: Word,
}

pub struct ReadResult {
    pub value: Word,
    pub proof: Proof,
}

pub type StepResult = Vec<Access>;
