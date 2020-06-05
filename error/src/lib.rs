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



//#![feature(try_trait)]
#![recursion_limit = "128"]

#[macro_use]
extern crate error_chain;
extern crate envy;
extern crate ethabi;
extern crate ethkey;
extern crate grpc;
extern crate hyper;
extern crate rustc_hex;
extern crate serde_yaml;
extern crate time;
extern crate web3;

use time::Duration;

error_chain! {
    foreign_links {
        Io(::std::io::Error) #[cfg(unix)];
        Parsing(serde_yaml::Error);
        Env(envy::Error);
        HexParse(rustc_hex::FromHexError);
        //EthAbi(ethabi::Error);
        JsonParse(serde_json::Error);
        Web3Contract(web3::contract::Error);
        LevelDB(leveldb::error::Error);
        Utf8(std::str::Utf8Error);
        Grpc(grpc::Error);
        Hyper(hyper::Error);
        Url(url::ParseError);
    }
    links {
        //Web3(web3::error::Error, web3::error::ErrorKind) #[cfg(unix)];
        EthAbiLink(ethabi::Error, ethabi::ErrorKind) #[cfg(unix)];
    }
    errors {
        Mpsc(details: String) {
            description("mspc send error")
                display("mspc send error: {}", details)
        }
        InvalidConfig(details: String) {
            description("invalid configuration")
                display("invalid configuration: {}", details)
        }
        ChainError(details: String) {
            description("blockchain presented error")
                display("blockchain presented error: {}",
                        details)
        }
        ChainNotInSync(delay: Duration, max_delay: Duration) {
            description("chain too delayed")
                display("ETH node not up to date: delay {}, max_delay {}",
                        delay,
                        max_delay)
        }
        InvalidTransactionRequest(details: String) {
            description("request of transaction invalid")
                display("request of transaction invalid: {}", details)
        }
        InvalidStateRequest(details: String) {
            description("request of state invalid")
                display("request of state invalid: {}", details)
        }
        InvalidContractState(details: String) {
            description("contract state invalid")
                display("contract state invalid: {}", details)
        }
        GrpcError(details: String) {
            description("error received from grpc")
                display("error received from grpc: {}", details)
        }
        ResponseMissError(service: String, key: String, method: String, request: Vec<u8>) {
            description("request data doesn't exist in archive")
                display("request data doesn't exist in archive, service: {}, key: {}, method: {}", service, key, method)
        }
        ResponseInvalidError(service: String, key: String, method: String) {
            description("request data in archive is invalid")
                display("request data in archive is invalid, service: {}, key: {}, method: {}", service, key, method)
        }
        ResponseNeedsDummy(service: String, key: String, method: String) {
            description("archive needs a dummy entry for the key")
                display("archive needs a dummy entry for the key, service: {}, key: {}, method: {}", service, key, method)
        }
        ServiceNeedsRetry(service: String, key: String, method: String, request: Vec<u8>, contract: String, status: u32, progress: u32, description: String) {
            description("service needs retry")
                display("service needs retry, service: {}, key: {}, method: {}, contract: {}, status: {}, progress: {}, description: {}", service, key, method, contract, status, progress, description)
        }
    }
}
