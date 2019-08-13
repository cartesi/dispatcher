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



extern crate ethereum_types;
extern crate futures;
extern crate futures_cpupool;
extern crate grpc;
extern crate keccak_hash;
extern crate protobuf;
extern crate rustc_hex;

use self::ethereum_types::H256;
use cartesi_base::*;
use grpc::SingleResponse;
use manager::*;
use manager_grpc::*;
use std::fmt;

pub struct HasherEmulator {
    defective: bool,
}

impl fmt::Display for HasherEmulator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Hasher {{ defective: {} }}", self.defective)
    }
}

impl HasherEmulator {
    pub fn new(defective: bool) -> Self {
        let hasher_emulator = HasherEmulator {
            defective: defective,
        };
        info!("Creating {}", hasher_emulator);
        return hasher_emulator;
    }
}

impl MachineManager for HasherEmulator {
    fn new_session(
        &self,
        _m: grpc::RequestOptions,
        _: NewSessionRequest,
    ) -> SingleResponse<Hash> {
        grpc::SingleResponse::completed(Hash::new())
    }
    fn session_run(
        &self,
        _m: grpc::RequestOptions,
        request: SessionRunRequest,
    ) -> SingleResponse<SessionRunResult> {
        let calculated_vec: Vec<Hash> =
            calculate_hasher_vector(&request.times, self.defective);
        info!(
            "Session {:?} received {:?} and returned {:?}",
            request.session_id, request.times, calculated_vec
        );
        let repeated_field = protobuf::RepeatedField::from_vec(calculated_vec);
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
        let value: u64 = request.time;
        let increased_value: u64 = request.time + 1;

        let siblings: Vec<Hash> = calculate_proof();

        let mut proof: Proof = Proof::new();
        proof.set_address(0);
        proof.set_log2_size(3);
        proof.set_sibling_hashes(protobuf::RepeatedField::from_vec(siblings));

        let mut access_read: Access = Access::new();
        access_read.set_operation(AccessOperation::READ);
        access_read.set_address(0);
        access_read.set_read(value);
        access_read.set_written(value);
        access_read.set_proof(proof.clone());

        let mut access_write: Access = Access::new();
        access_write.set_operation(AccessOperation::WRITE);
        access_write.set_address(0);
        access_write.set_read(value);
        access_write.set_written(increased_value);
        access_write.set_proof(proof);

        let mut access_log: AccessLog = AccessLog::new();
        access_log.set_accesses(protobuf::RepeatedField::from_vec(vec![
            access_read,
            access_write,
        ]));
        let mut r = SessionStepResult::new();
        r.set_log(access_log);
        grpc::SingleResponse::completed(r)
    }
}

fn calculate_hasher_vector(times: &Vec<u64>, defective: bool) -> Vec<Hash> {
    let mut uncles: Vec<H256> = Vec::new();
    uncles.push(calculate_hash_u64(0));
    for i in 1..61 {
        let previous = uncles[i - 1].clone();
        uncles.push(calculate_hash_pair(previous, previous));
    }
    return times
        .into_iter()
        .map(move |time| {
            let altered_time: u64;
            if defective {
                altered_time = std::cmp::min(*time, 17);
            } else {
                altered_time = *time;
            }
            let mut running_hash = calculate_hash_u64(altered_time);
            for i in 0..61 {
                running_hash = calculate_hash_pair(running_hash, uncles[i]);
            }
            let mut returned_hash = Hash::new();
            returned_hash.set_content(running_hash.to_vec());
            returned_hash.clone()
        })
        .collect();
}

fn calculate_proof() -> Vec<Hash> {
    let mut uncles: Vec<H256> = Vec::new();
    uncles.push(calculate_hash_u64(0));
    for i in 1..61 {
        let previous = uncles[i - 1].clone();
        uncles.push(calculate_hash_pair(previous, previous));
    }
    return uncles
        .into_iter()
        .map(|hash| {
            let mut result = Hash::new();
            result.set_content(hash.0.to_vec());
            result
        })
        .collect();
}

fn calculate_hash_u64(data: u64) -> H256 {
    let bytes: [u8; 8] = data.to_be_bytes();
    return keccak_hash::keccak(&bytes);
}

fn calculate_hash_pair(data_1: H256, data_2: H256) -> H256 {
    let bytes_1: [u8; 32] = data_1.into();
    let bytes_2: [u8; 32] = data_2.into();
    let mut vec_1: Vec<u8> = bytes_1.into_iter().map(|&a| a).collect();
    let mut vec_2: Vec<u8> = bytes_2.into_iter().map(|&a| a).collect();
    vec_1.append(&mut vec_2);
    return keccak_hash::keccak(&vec_1.as_slice());
}
