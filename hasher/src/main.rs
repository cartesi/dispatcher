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
use std::env;
use std::thread;

fn main() {
    env_logger::init();

    let mut arguments = std::env::args();
    let port: u16 = arguments
        .nth(1)
        .unwrap_or("50051".to_string())
        .parse()
        .expect("could not parse port");
    let fake: bool = arguments
        .next()
        .unwrap_or("false".to_string())
        .parse()
        .expect("could not parse fakeness");
    //let port = 50051;
    let mut server = grpc::ServerBuilder::new_plain();
    server.http.set_port(port);
    let hasher_emulator = HasherEmulator::new(fake);
    server.add_service(EmulatorServer::new_service_def(hasher_emulator));
    server.http.set_cpu_pool_threads(1);
    let _server = server.build().expect("server");
    info!(
        "Greeter server started on port {} and fakeness is {}",
        port, fake
    );

    loop {
        thread::park();
    }
}
