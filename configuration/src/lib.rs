//extern crate ethabi;

const DEFAULT_MAX_DELAY: u64 = 100;

pub struct Concern {
    contract: String,
    user: String,
}

//#[macro_use]
//extern crate serde_derive;

//#[derive(Serialize, Deserialize)]
pub struct Configuration {
    max_delay: Option<u64>,
    pub concerns: Vec<Concern>,
}

impl Configuration {
    pub fn new(path: &str) -> Configuration {
        Configuration {
            max_delay: None,
            concerns: vec![],
        }
    }
    pub fn get_max_delay(&self) -> u64 {
        self.max_delay.unwrap_or(DEFAULT_MAX_DELAY)
    }
}
