// error-chain recursion
#![recursion_limit = "1024"]

#[macro_use]
extern crate log;
extern crate dispatcher;
extern crate env_logger;

use dispatcher::Dispatcher;

fn main() {
    env_logger::init();

    if let Err(ref e) = Dispatcher::new("config.yaml") {
        error!("error: {}", e);

        for e in e.iter().skip(1) {
            error!("caused by: {}", e);
        }

        // The backtrace is not always generated. Try to run this example
        // with `RUST_BACKTRACE=1`.
        if let Some(backtrace) = e.backtrace() {
            error!("backtrace: {:?}", backtrace);
        }

        ::std::process::exit(1);
    }

    // let accounts = web3.eth().accounts().wait()?;
    // println!("Account: {:?}", accounts[0]);
    // let block_number = web3.eth().block_number().wait()?.low_u64();
    // println!("Block number: {:?}", block_number);
}
