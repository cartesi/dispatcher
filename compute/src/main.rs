// error-chain recursion
#![recursion_limit = "1024"]

#[macro_use]
extern crate log;
extern crate compute;
extern crate dispatcher;
extern crate env_logger;
extern crate error;

use compute::Compute;
use dispatcher::Dispatcher;
use error::*;

fn print_error(e: &Error) {
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

fn main() {
    env_logger::init();

    let dispatcher = match Dispatcher::new() {
        Ok(d) => d,
        Err(ref e) => {
            print_error(e);
            return;
        }
    };

    if let Err(ref e) = dispatcher.run::<Compute>() {
        print_error(e);
    }
    // let accounts = web3.eth().accounts().wait()?;
    // println!("Account: {:?}", accounts[0]);
    // let block_number = web3.eth().block_number().wait()?.as_u64();
    // println!("Block number: {:?}", block_number);
}
