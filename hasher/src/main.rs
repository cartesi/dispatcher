// Note: This component currently has dependencies that are licensed under the GNU GPL, version 3, and so you should treat this component as a whole as being under the GPL version 3. But all Cartesi-written code in this component is licensed under the Apache License, version 2, or a compatible permissive license, and can be used independently under the Apache v2 license. After this component is rewritten, the entire component will be released under the Apache v2 license.

// Copyright 2019 Cartesi Pte. Ltd.

// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.




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
