extern crate ethereum_types;
extern crate futures;
extern crate futures_cpupool;
extern crate grpc;
extern crate protobuf;

use self::ethereum_types::{H256, U256};
use emu::*;
use emu_grpc::*;
use grpc::SingleResponse;

pub struct HasherEmulator {
    fake: bool,
}

fn calculate_hasher_vector(
    input_hash: String,
    times: Vec<u64>,
    fake: bool,
) -> Vec<Hash> {
    let hash: String = input_hash.chars().take_while(|&s| s != ':').collect();
    return times
        .into_iter()
        .map(|time| {
            let mut returned_hash = Hash::new();
            let u: u64;
            if fake {
                u = hash
                    .parse::<U256>()
                    .expect("could not parse u256")
                    .low_u64()
                    + std::cmp::min(time, 17);
            } else {
                u = hash
                    .parse::<U256>()
                    .expect("could not parse u256")
                    .low_u64()
                    + time;
            }
            returned_hash
                .set_hash(format!("{:x}", H256::from(U256::from(u))).into());
            returned_hash.clone()
        })
        .collect();
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
        info!("Running session {:?}", request.session);
        let initial_hash = request.session.into_option().unwrap().id;
        let v: Vec<Hash> =
            calculate_hasher_vector(initial_hash, request.times, self.fake);
        // let v: Vec<_> =
        //     request.times.iter().map(move |_| hash.clone()).collect();
        //    vec![hash.clone(), hash.clone()];
        info!("Session returned {:?}", v);
        let repeated_field = protobuf::RepeatedField::from_vec(v);
        let mut r = RunResult::new();
        r.set_hashes(repeated_field);
        grpc::SingleResponse::completed(r)
    }
    fn step(
        &self,
        _m: grpc::RequestOptions,
        _: StepRequest,
    ) -> SingleResponse<StepResult> {
        let mut r = StepResult::new();
        r.set_response(protobuf::RepeatedField::from_vec(vec![]));
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
