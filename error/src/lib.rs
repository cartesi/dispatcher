#[macro_use]
extern crate error_chain;
extern crate envy;
extern crate ethkey;
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
    }
    links {
        Web3(web3::error::Error, web3::error::ErrorKind) #[cfg(unix)];
    }
    errors {
        InvalidConfig(details: String) {
            description("invalid configuration")
                display("invalid configuration: {}", details)
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
    }
}
