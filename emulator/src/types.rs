extern crate configuration;

use self::configuration::Configuration;
use super::error::*;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Backing {
    Zeros,
    Path(PathBuf),
}

pub type Word = [u8; 8];

#[derive(Debug)]
pub struct Ram {
    pub ilength: Word,
    pub backing: Backing,
}

#[derive(Debug)]
pub struct Drive {
    pub istart: Word,
    pub ilength: Word, // should this be the log_2 of the length?
    pub backing: Backing,
}

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
pub struct Proof {
    pub address: Word,
    pub depth: u32,
    pub root: Hash,
    pub siblings: Vec<Hash>,
    pub target: Hash,
}

#[derive(Debug)]
pub enum Operation {
    Read,
    Write,
}

#[derive(Debug)]
pub struct Access {
    pub operation: Operation,
    pub read: Word,
    pub written: Word,
    pub proof: Proof,
}

pub type SessionId = String;

#[derive(Debug)]
pub struct InitRequest {
    pub session: SessionId,
    pub machine: MachineSpecification,
}

#[derive(Debug)]
pub struct RunRequest {
    pub session: SessionId,
    pub times: Vec<u64>,
}

#[derive(Debug)]
pub struct StepRequest {
    pub session: SessionId,
    pub time: u64,
}

#[derive(Debug)]
pub struct DriveRequest {
    pub session: SessionId,
    pub drive: DriveId,
}

#[derive(Debug)]
pub struct ReadRequest {
    pub session: SessionId,
    pub address: Word,
}

#[derive(Debug)]
pub struct ReadResult {
    pub value: Word,
    pub proof: Proof,
}

pub type StepResult = Vec<Access>;
