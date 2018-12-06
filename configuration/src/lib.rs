extern crate ethabi;

pub struct Concern {
    contract: String,
    user: String,
}

pub struct Configuration {
    concerns: Vec<Concern>,
}
