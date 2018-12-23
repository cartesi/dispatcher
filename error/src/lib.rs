#[macro_use]
extern crate error_chain;
extern crate envy;
extern crate serde_yaml;
extern crate web3;

error_chain!{
    foreign_links {
        Io(::std::io::Error) #[cfg(unix)];
        Parsing(serde_yaml::Error);
        Env(envy::Error);
    }
    links {
        Web3(web3::error::Error, web3::error::ErrorKind) #[cfg(unix)];
    }
    errors {
        InvalidConfig(details: String) {
            description("invalid configuration")
                display("invalid configuration: {}", details)
        }
        ChainNotInSync(delay: i64, max_delay: u64) {
            description("chain too delayed")
                display("ETH node not up to date: delay {}, max_delay {}",
                        delay,
                        max_delay)
        }
    }
}
