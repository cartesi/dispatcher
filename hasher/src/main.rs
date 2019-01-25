extern crate env_logger;
extern crate futures;
extern crate futures_cpupool;
extern crate grpc;
#[macro_use]
extern crate log;
extern crate protobuf;

pub mod emu;
pub mod emu_grpc;
pub mod hasher;

use emu::*;
use emu_grpc::*;
use grpc::SingleResponse;
use hasher::HasherEmulator;
use std::thread;

fn main() {
    env_logger::init();

    let port = 50051;
    let mut server = grpc::ServerBuilder::new_plain();
    server.http.set_port(port);
    server.add_service(EmulatorServer::new_service_def(HasherEmulator));
    server.http.set_cpu_pool_threads(1);
    let _server = server.build().expect("server");
    info!("greeter server started on port {}", port);

    loop {
        thread::park();
    }
}
