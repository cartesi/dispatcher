extern crate web3;

use std::time::{SystemTime, UNIX_EPOCH};
use web3::error::{Error, ErrorKind};
use web3::futures::Future;
use web3::types::H256;
use web3::types::{Block, BlockId, BlockNumber};
use web3::Transport;

fn str_error(msg: &str) -> Error {
    ErrorKind::Msg(String::from(msg)).into()
}

pub trait EthExt<T: Transport> {
    fn get_delay(&self) -> Box<Future<Item = i64, Error = Error>>;
}

impl<T: Transport + 'static> EthExt<T> for web3::api::Eth<T> {
    fn get_delay(&self) -> Box<Future<Item = i64, Error = Error>> {
        Box::new(self.block(BlockId::Number(BlockNumber::Latest)).then(
            |block_result: Result<Option<Block<H256>>, Error>| {
                let block_time: i64 = block_result?
                    .ok_or(str_error("Latest block not found"))?
                    .timestamp
                    .low_u64() as i64;
                let current_time: i64 = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_e| str_error("Time went backwards"))?
                    .as_secs() as i64;
                Ok(current_time - block_time)
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
