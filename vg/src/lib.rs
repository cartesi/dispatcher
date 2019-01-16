#![feature(proc_macro_hygiene, generators, transpose_result)]

extern crate configuration;
extern crate env_logger;
extern crate error;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate dispatcher;
extern crate ethabi;
extern crate ethereum_types;
extern crate serde_json;
extern crate state;

use configuration::{Concern, Configuration};
use dispatcher::{Archive, DApp};
use error::*;
use ethabi::Token;
use ethereum_types::{Address, U256};
use serde_json::Value;
use state::Instance;

pub struct VG();

impl VG {
    pub fn new() -> Self {
        VG()
    }
}

impl DApp for VG {
    fn react(&self, instance: state::Instance, archive: Archive) -> String {
        return String::from("HA!");
    }
}
