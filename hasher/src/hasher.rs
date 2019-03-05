extern crate ethereum_types;
extern crate futures;
extern crate futures_cpupool;
extern crate grpc;
extern crate keccak_hash;
extern crate protobuf;
extern crate rustc_hex;

use self::ethereum_types::{H256, U256};
use self::rustc_hex::{FromHex, ToHex};
use emu::*;
use emu_grpc::*;
use futures::Future;
use grpc::SingleResponse;

pub struct HasherEmulator {
    fake: bool,
}

impl HasherEmulator {
    pub fn new(fake: bool) -> Self {
        info!("Creating new hasher with fakeness: {}", fake);
        HasherEmulator { fake: fake }
    }
}

impl Emulator for HasherEmulator {
    fn init(
        &self,
        _m: grpc::RequestOptions,
        _: InitRequest,
    ) -> SingleResponse<Hash> {
        let mut r = Hash::new();
        r.set_hash("Not implemented!".into());
        grpc::SingleResponse::completed(r)
    }
    fn run(
        &self,
        _m: grpc::RequestOptions,
        request: RunRequest,
    ) -> SingleResponse<RunResult> {
        let v: Vec<Hash> = calculate_hasher_vector(&request.times, self.fake);
        info!(
            "Session {:?} received {:?} and returned {:?}",
            request.session, request.times, v
        );
        let repeated_field = protobuf::RepeatedField::from_vec(v);
        let mut r = RunResult::new();
        r.set_hashes(repeated_field);
        grpc::SingleResponse::completed(r)
    }
    fn step(
        &self,
        _m: grpc::RequestOptions,
        request: StepRequest,
    ) -> SingleResponse<StepResult> {
        info!(
            "Session {:?} received {:?} and returned a proof",
            request.session, request.time
        );
        let mut address: Word = Word::new();
        address.set_word(0);
        let mut value: Word = Word::new();
        value.set_word(request.time);
        let mut increased_value: Word = Word::new();
        increased_value.set_word(request.time + 1);

        let siblings: Vec<Hash> = calculate_proof(request.time, self.fake)
            .into_iter()
            .map(|hash| {
                let mut result: Hash = Hash::new();
                result.set_hash(hash.to_hex());
                result
            })
            .collect();

        let mut proof: Proof = Proof::new();
        proof.set_address(address.clone());
        proof.set_depth(61);
        proof.set_siblings(protobuf::RepeatedField::from_vec(siblings));

        let mut access_read: Access = Access::new();
        access_read.set_operation(Access_Operation::READ);
        access_read.set_address(address.clone());
        access_read.set_value_before(value.clone());
        access_read.set_value_after(value.clone());
        access_read.set_proof(proof.clone());

        let mut access_write: Access = Access::new();
        access_write.set_operation(Access_Operation::WRITE);
        access_write.set_address(address);
        access_write.set_value_before(value);
        access_write.set_value_after(increased_value);
        access_write.set_proof(proof);

        //           proof.into_iter().map(|hash| hash).collect();
        let mut r = StepResult::new();
        r.set_response(protobuf::RepeatedField::from_vec(vec![
            access_read,
            access_write,
        ]));
        grpc::SingleResponse::completed(r)
    }
    fn read(
        &self,
        _m: grpc::RequestOptions,
        _: ReadRequest,
    ) -> SingleResponse<ReadResult> {
        let r = ReadResult::new();
        grpc::SingleResponse::completed(r)
    }
    fn prove_drive(
        &self,
        _m: grpc::RequestOptions,
        _: DriveRequest,
    ) -> SingleResponse<Proof> {
        let r = Proof::new();
        grpc::SingleResponse::completed(r)
    }
    fn get_backing(
        &self,
        _m: grpc::RequestOptions,
        _: DriveRequest,
    ) -> SingleResponse<Backing> {
        let r = Backing::new();
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
            returned_hash.set_hash(format!("0x{}", hex_answer));
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
