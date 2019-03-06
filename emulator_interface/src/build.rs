extern crate protoc_rust_grpc;

fn main() {
    //match
    protoc_rust_grpc::run(protoc_rust_grpc::Args {
        out_dir: "src",
        includes: &["../cartesi-grpc"],
        input: &[
            "../cartesi-grpc/cartesi-base.proto",
            "../cartesi-grpc/manager.proto",
        ],
        rust_protobuf: true, // generate protobuf messages, not just services
        ..Default::default()
    })
    .expect("protoc-rust-grpc");
}
