extern crate configuration;
extern crate utils;
extern crate web3;

use configuration::Configuration;
use utils::EthExt;
use web3::futures::Future;

fn main() {
    //let url = "http://35.231.249.221:8545"; // remote parity
    let url = "http://127.0.0.1:8545"; // ganache

    let config = Configuration::new("bla");
    println!("Max delay: {}", config.get_max_delay());

    let (_eloop, transport) = web3::transports::Http::new(url).unwrap();
    let web3 = web3::Web3::new(transport);
    let accounts = web3.eth().accounts().wait().unwrap();

    println!("Accounts: {:?}", accounts);

    let block_number = web3.eth().block_number().wait().unwrap().low_u64();
    println!("Block number: {:?}", block_number);

    let a = web3.eth().get_delay().wait().unwrap();

    println!("Delay: {:?}", a);
}
