extern crate ethereum_types;
extern crate futures;
extern crate futures_cpupool;
extern crate grpc;
extern crate keccak_hash;
extern crate protobuf;
extern crate rustc_hex;

use self::ethereum_types::{H256, U256};
use self::rustc_hex::{FromHex, ToHex};
use cartesi_base::*;
use futures::Future;
use grpc::SingleResponse;
use manager::*;
use manager_grpc::*;

pub struct HasherEmulator {
    fake: bool,
}

impl HasherEmulator {
    pub fn new(fake: bool) -> Self {
        info!("Creating new hasher with fakeness: {}", fake);
        HasherEmulator { fake: fake }
    }
}

impl MachineManager for HasherEmulator {
    fn new_session(
        &self,
        _m: grpc::RequestOptions,
        _: NewSessionRequest,
    ) -> SingleResponse<Hash> {
        let mut r = Hash::new();
        r.set_content(vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        grpc::SingleResponse::completed(r)
    }
    fn session_run(
        &self,
        _m: grpc::RequestOptions,
        request: SessionRunRequest,
    ) -> SingleResponse<SessionRunResult> {
        let v: Vec<Hash> = calculate_hasher_vector(&request.times, self.fake);
        info!(
            "Session {:?} received {:?} and returned {:?}",
            request.session_id, request.times, v
        );
        let repeated_field = protobuf::RepeatedField::from_vec(v);
        let mut r = SessionRunResult::new();
        r.set_hashes(repeated_field);
        grpc::SingleResponse::completed(r)
    }
    fn session_step(
        &self,
        _m: grpc::RequestOptions,
        request: SessionStepRequest,
    ) -> SingleResponse<SessionStepResult> {
        info!(
            "Session {:?} received {:?} and returned a proof",
            request.session_id, request.time
        );
        let value: U256 = U256::from(request.time);
        let increased_value: U256 = U256::from(request.time + 1);

        let siblings: Vec<Hash> = calculate_proof(request.time, self.fake)
            .into_iter()
            .map(|hash| {
                let mut result = Hash::new();
                result.set_content(hash.0.to_vec());
                result
            })
            .collect();

        let mut proof: Proof = Proof::new();
        proof.set_address(0);
        proof.set_log2_size(3);
        proof.set_sibling_hashes(protobuf::RepeatedField::from_vec(siblings));

        let mut access_read: Access = Access::new();
        access_read.set_operation(AccessOperation::READ);
        access_read.set_address(0);
        access_read.set_read(value.clone().as_u64());
        access_read.set_written(value.clone().as_u64());
        access_read.set_proof(proof.clone());

        let mut access_write: Access = Access::new();
        access_write.set_operation(AccessOperation::WRITE);
        access_write.set_address(0);
        access_write.set_read(value.as_u64());
        access_write.set_written(increased_value.as_u64());
        access_write.set_proof(proof);

        let mut access_log: AccessLog = AccessLog::new();
        access_log.set_accesses(protobuf::RepeatedField::from_vec(vec![
            access_read,
            access_write,
        ]));

        //           proof.into_iter().map(|hash| hash).collect();
        let mut r = SessionStepResult::new();
        r.set_log(access_log);
        grpc::SingleResponse::completed(r)
    }
}

fn calculate_hasher_vector(times: &Vec<u64>, fake: bool) -> Vec<Hash> {
    let binary_0 = H256::from(0);
    let mut uncles: Vec<H256> = Vec::new();
    uncles.push(binary_0);
    for i in 1..61 {
        let previous = uncles[i - 1].clone();
        uncles.push(calculate_hash_pair(previous, previous));
    }
    return times
        .into_iter()
        .map(move |time| {
            let mut returned_hash = Hash::new();
            let u: u64;
            if fake {
                u = std::cmp::min(*time, 17);
            } else {
                u = *time;
            }
            let binary_u = H256::from(u);
            let mut running_hash = calculate_hash(binary_u);
            for i in 0..61 {
                running_hash = calculate_hash_pair(running_hash, uncles[i]);
            }
            let hex_answer: String = running_hash.to_hex();
            warn!("{}", hex_answer);
            returned_hash.set_content(hex_answer.as_bytes().to_vec());
            returned_hash.clone()
        })
        .collect();
}

fn calculate_proof(time: u64, fake: bool) -> Vec<H256> {
    let new_time = if ((time == 17) & fake) { 16 } else { time };
    let binary_0 = H256::from(0);
    let mut uncles: Vec<H256> = Vec::new();
    uncles.push(binary_0);
    for i in 1..61 {
        let previous = uncles[i - 1].clone();
        uncles.push(calculate_hash_pair(previous, previous));
    }
    return uncles;
}

fn calculate_hash(data: H256) -> H256 {
    let hex: String = data.to_hex();
    let bytes: Vec<u8> = hex.from_hex().unwrap();
    let hex_hash: String = keccak_hash::keccak(&bytes).to_hex();
    return hex_hash.parse().unwrap();
}

fn calculate_hash_pair(data_1: H256, data_2: H256) -> H256 {
    let hex_1: String = data_1.to_hex();
    let hex_2: String = data_2.to_hex();
    let hex = format!("{}{}", hex_1, hex_2);
    let bytes: Vec<u8> = hex.from_hex().unwrap();
    let hex_hash: String = keccak_hash::keccak(&bytes).to_hex();
    return hex_hash.parse().unwrap();
}
