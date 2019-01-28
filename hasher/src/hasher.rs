extern crate futures;
extern crate futures_cpupool;
extern crate grpc;
extern crate protobuf;

use emu::*;
use emu_grpc::*;
use grpc::SingleResponse;

pub struct HasherEmulator;

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
        let mut hash = Hash::new();
        hash.set_hash(
            "0x0000000000000000000000000000000000000000000000000000000000000000"
                .into(),
        );
        let v: Vec<_> =
            request.times.iter().map(move |_| hash.clone()).collect();
        //    vec![hash.clone(), hash.clone()];
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
