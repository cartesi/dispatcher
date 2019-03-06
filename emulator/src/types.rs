extern crate configuration;
extern crate emulator_interface;
extern crate ethereum_types;
extern crate rustc_hex;

use self::configuration::Configuration;
use self::ethereum_types::{H256, U256};
use self::rustc_hex::FromHex;
use super::error::*;
use emulator_interface::cartesi_base;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SessionRunRequest {
    pub session_id: String,
    pub times: Vec<u64>,
}

impl From<emulator_interface::manager::SessionRunRequest>
    for SessionRunRequest
{
    fn from(result: emulator_interface::manager::SessionRunRequest) -> Self {
        SessionRunRequest {
            session_id: result.session_id,
            times: result.times,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionRunResult {
    pub hashes: Vec<H256>,
}

impl From<emulator_interface::manager::SessionRunResult> for SessionRunResult {
    fn from(result: emulator_interface::manager::SessionRunResult) -> Self {
        SessionRunResult {
            hashes: result
                .hashes
                .into_vec()
                .into_iter()
                .map(|hash| H256::from_slice(&hash.content))
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Proof {
    pub address: u64,
    pub log2_size: u32,
    pub target_hash: H256,
    pub sibling_hashes: Vec<H256>,
    pub root_hash: H256,
}

#[derive(Debug, Clone)]
pub enum AccessOperation {
    Read,
    Write,
}

impl From<cartesi_base::AccessOperation> for AccessOperation {
    fn from(op: cartesi_base::AccessOperation) -> Self {
        match op {
            cartesi_base::AccessOperation::READ => AccessOperation::Read,
            cartesi_base::AccessOperation::WRITE => AccessOperation::Write,
        }
    }
}

impl From<cartesi_base::Proof> for Proof {
    fn from(proof: cartesi_base::Proof) -> Self {
        Proof {
            address: proof.address,
            log2_size: proof.log2_size,
            target_hash: H256::from_slice(
                &proof
                    .target_hash
                    .into_option()
                    .expect("target hash not found")
                    .content,
            ),
            sibling_hashes: proof
                .sibling_hashes
                .into_vec()
                .into_iter()
                .map(|hash| H256::from_slice(&hash.content))
                .collect(),
            root_hash: H256::from_slice(
                &proof
                    .root_hash
                    .into_option()
                    .expect("root hash not found")
                    .content,
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Access {
    pub operation: AccessOperation,
    pub address: u64,
    pub value_read: u64,
    pub value_written: u64,
    pub proof: Proof,
}

impl From<emulator_interface::cartesi_base::Access> for Access {
    fn from(access: emulator_interface::cartesi_base::Access) -> Self {
        Access {
            operation: access.operation.into(),
            address: access.address,
            value_read: access.read,
            value_written: access.written,
            proof: access.proof.into_option().expect("proof not found").into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionStepRequest {
    pub session_id: String,
    pub time: u64,
}

impl From<emulator_interface::manager::SessionStepRequest>
    for SessionStepRequest
{
    fn from(result: emulator_interface::manager::SessionStepRequest) -> Self {
        SessionStepRequest {
            session_id: result.session_id,
            time: result.time,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionStepResult {
    pub log: Vec<Access>,
}

impl From<emulator_interface::manager::SessionStepResult>
    for SessionStepResult
{
    fn from(result: emulator_interface::manager::SessionStepResult) -> Self {
        SessionStepResult {
            log: result
                .log
                .into_option()
                .expect("log not found")
                .accesses
                .into_vec()
                .into_iter()
                .map(|hash| hash.into())
                .collect(),
        }
    }
}
