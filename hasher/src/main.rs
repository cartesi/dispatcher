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



extern crate env_logger;
extern crate futures;
extern crate futures_cpupool;
extern crate grpc;
extern crate manager;
extern crate manager_grpc;

#[macro_use]
extern crate log;
extern crate protobuf;

pub mod cartesi_base;
pub mod hasher;

use hasher::HasherEmulator;
use manager_grpc::*;
use std::thread;

fn main() {
    env_logger::init();

    let mut arguments = std::env::args();
    let port: u16 = arguments
        .nth(1)
        .unwrap_or("50051".to_string())
        .parse()
        .expect("could not parse port");
    let defective: bool = arguments
        .next()
        .unwrap_or("false".to_string())
        .parse()
        .expect("could not parse defectiveness");
    let mut server = grpc::ServerBuilder::new_plain();
    server.http.set_port(port);
    let hasher_emulator = HasherEmulator::new(defective);
    server.add_service(MachineManagerServer::new_service_def(hasher_emulator));
    server.http.set_cpu_pool_threads(1);
    let _server = server.build().expect("server");
    info!(
        "Greeter server started on port {} and defectiveness is {}",
        port, defective
    );

    loop {
        thread::park();
    }
}
