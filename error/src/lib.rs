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
        EthKey(ethkey::Error);
        HexParse(rustc_hex::FromHexError);
        //EthAbi(ethabi::Error);
        JsonParse(serde_json::Error);
        Web3Contract(web3::contract::Error);
        LevelDB(leveldb::error::Error);
        Utf8(std::str::Utf8Error);
        Grpc(grpc::Error);
        Hyper(hyper::Error);
    }
    links {
        //Web3(web3::error::Error, web3::error::ErrorKind) #[cfg(unix)];
        Web3(web3::Error, web3::error::ErrorKind) #[cfg(unix)];
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
    }
}
