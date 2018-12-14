#[macro_use]
extern crate error_chain;
extern crate serde_yaml;
extern crate web3;

error_chain!{
    foreign_links {
        Io(::std::io::Error) #[cfg(unix)];
        Parsing(serde_yaml::Error);
    }
    links {
        Web3(web3::error::Error, web3::error::ErrorKind) #[cfg(unix)];
    }
    errors {
        InvalidConfig {
            description("invalid max_delay and warn_delay")
                display("both max_delay and warn_delay must be set and valid")
        }
        ChainNotInSync(delay: i64, max_delay: u64) {
            description("chain too delayed")
                display("ETH node not up to date: delay {}, max_delay {}",
                        delay,
                        max_delay)
        }
    }
}
