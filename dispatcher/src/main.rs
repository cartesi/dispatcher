extern crate utils;
extern crate web3;

use utils::EthExt;
use web3::futures::Future;

fn main() {
    let (_eloop, transport) =
        web3::transports::Http::new("http://35.231.249.221:8545").unwrap();
    let web3 = web3::Web3::new(transport);
    let accounts = web3.eth().accounts().wait().unwrap();

    println!("Accounts: {:?}", accounts);

    let block_number = web3.eth().block_number().wait().unwrap().low_u64();
    println!("Block number: {:?}", block_number);

    let a = web3.eth().get_delay().wait().unwrap();

    println!("Delay: {:?}", a);
}
