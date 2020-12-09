extern crate env_logger;
#[macro_use]
extern crate log;
extern crate error;
extern crate ethabi;
extern crate ethereum_types;
extern crate parity_crypto;
extern crate serde;
extern crate serde_json;
extern crate tokio;
extern crate transport;
extern crate web3;

use error::*;
use ethereum_types::{Address, U256};
use parity_crypto::publickey::KeyPair;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use transport::GenericTransport;
use web3::futures::Future;

/// Either a key pair, or a single address, to be used either to sign a
/// transaction, or to send an unsigned transaction to an external signer.
#[derive(Clone, Debug)]
pub enum ConcernKey {
    KeyPair(KeyPair),
    UserAddress(Address),
}

impl ConcernKey {
    pub fn address(&self) -> Address {
        match self {
            ConcernKey::KeyPair(key_pair) => key_pair.address(),
            ConcernKey::UserAddress(address) => address.clone(),
        }
    }
}

/// A worker node, with its ABI and address
#[derive(Debug, Clone)]
pub struct Worker {
    abi: PathBuf,
    contract_address: Address,
    key: ConcernKey,
}

#[derive(Debug, Clone)]
enum WorkerState {
    Available,
    Pending(Address),
    Owned(Address),
    Retired(Address),
}

impl Worker {
    pub fn new(
        abi: PathBuf,
        contract_address: Address,
        key: ConcernKey,
    ) -> Self {
        Worker {
            abi,
            contract_address,
            key,
        }
    }
    // Worker accept job if needed
    pub fn accept_job(
        &self,
        web3: &web3::Web3<GenericTransport>,
    ) -> Result<Address> {
        trace!("Start accept job");
        let (contract, abi) = self.build_contract_abi(web3)?;

        loop {
            trace!("Getting worker state");
            let worker_state = self.get_worker_state(&contract)?;
            trace!("Worker state: {:?}", worker_state);

            match worker_state {
                WorkerState::Available => (),
                WorkerState::Pending(_) => {
                    // Accept job
                    self.send_accept_job(web3, &abi)?;
                }
                WorkerState::Owned(owner_address) => {
                    return Ok(owner_address);
                }
                WorkerState::Retired(owner_address) => {
                    // If worker is retired, transfer all funds back to user_address, and end
                    // dispatcher with a Ok return value.
                    self.transfer_worker_funds(&web3, &owner_address)?;
                    std::process::exit(0)
                }
            }

            // Wait 15 seconds before trying again.
            std::thread::sleep(std::time::Duration::from_secs(15));
        }
    }

    pub fn poll_worker_status(
        &self,
        web3: &web3::Web3<GenericTransport>,
    ) -> Result<()> {
        info!("Start polling worker status");
        let (contract, _) = self.build_contract_abi(web3)?;

        loop {
            trace!("Getting worker state");
            let worker_state = self.get_worker_state(&contract)?;
            trace!("Worker state: {:?}", worker_state);

            match worker_state {
                WorkerState::Retired(owner_address) => {
                    // If worker is retired, transfer all funds back to user_address, and end
                    // dispatcher with a Ok return value.
                    self.transfer_worker_funds(&web3, &owner_address)?;
                    return Ok(());
                }
                _ => {}
            }

            // Wait 15 seconds before trying again.
            std::thread::sleep(std::time::Duration::from_secs(15));
        }
    }

    fn build_contract_abi(
        &self,
        web3: &web3::Web3<GenericTransport>,
    ) -> Result<(web3::contract::Contract<GenericTransport>, ethabi::Contract)>
    {
        // Load abi
        let mut file = File::open(self.abi.clone())?;
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        let v: serde_json::Value = serde_json::from_str(&s[..])
            .chain_err(|| format!("could not read truffle json file"))?;

        // create a contract object
        let contract = web3::contract::Contract::from_json(
            web3.eth().clone(),
            self.contract_address,
            serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
        )
        .chain_err(|| format!("could not create contract for worker"))?;

        // Create a low level abi for worker contract
        let abi = ethabi::Contract::load(
            serde_json::to_string(&v["abi"]).unwrap().as_bytes(),
        )
        .chain_err(|| format!("Could not create low level abi for worker"))?;

        Ok((contract, abi))
    }

    fn get_worker_state(
        &self,
        contract: &web3::contract::Contract<GenericTransport>,
    ) -> Result<WorkerState> {
        let (query_result, user_address): (_, Address) =
            web3::futures::future::join_all(vec![
                self.build_worker_state_query("isAvailable", contract),
                self.build_worker_state_query("isPending", contract),
                self.build_worker_state_query("isOwned", contract),
                self.build_worker_state_query("isRetired", contract),
            ])
            .join(contract.query(
                "getUser",
                self.key.address(),
                None,
                web3::contract::Options::default(),
                None,
            ))
            .wait()?;

        let query_result = (
            query_result[0],
            query_result[1],
            query_result[2],
            query_result[3],
        );

        match query_result {
            (true, _, _, _) => Ok(WorkerState::Available),
            (_, true, _, _) => Ok(WorkerState::Pending(user_address)),
            (_, _, true, _) => Ok(WorkerState::Owned(user_address)),
            (_, _, _, true) => Ok(WorkerState::Retired(user_address)),
            (false, false, false, false) => {
                Err(Error::from(format!("Invalid blockchain state")))
            }
        }
    }

    fn build_worker_state_query(
        &self,
        func: &str,
        contract: &web3::contract::Contract<GenericTransport>,
    ) -> web3::contract::QueryResult<
        bool,
        std::boxed::Box<
            dyn tokio::prelude::Future<
                    Item = serde_json::Value,
                    Error = web3::Error,
                > + std::marker::Send,
        >,
    > {
        contract.query(
            func,
            self.key.address(),
            None,
            web3::contract::Options::default(),
            None,
        )
    }

    fn send_accept_job(
        &self,
        web3: &web3::Web3<GenericTransport>,
        abi: &ethabi::Contract,
    ) -> Result<()> {
        trace!("Accept job transaction...");
        let gas_price = web3.eth().gas_price().wait()?;
        let gas_price = U256::from(2).checked_mul(gas_price).unwrap();

        abi.function("acceptJob")
            .and_then(|function| function.encode_input(&vec![]))
            .map(move |data| {
                match &self.key {
                    ConcernKey::UserAddress(address) => {
                        let tx_request = web3::types::TransactionRequest {
                            from: *address,
                            to: Some(self.contract_address),
                            gas_price: Some(gas_price),
                            gas: None,
                            value: None,
                            data: Some(web3::types::Bytes(data)),
                            condition: None,
                            nonce: None,
                        };

                        trace!("Sending unsigned transaction");
                        web3.eth()
                            .send_transaction(tx_request)
                            .map(|hash| {
                                info!("Transaction sent with hash: {:?}", hash);
                            })
                            .or_else(|e| {
                                // ignore the nonce error, by pass the other errors
                                if let web3::error::Error::Rpc(ref rpc_error) =
                                    e
                                {
                                    let nonce_error = String::from(
                                        "the tx doesn't have the correct nonce",
                                    );
                                    if rpc_error.message[..nonce_error.len()]
                                        == nonce_error
                                    {
                                        warn!(
                                            "Ignoring nonce Error: {}",
                                            rpc_error.message
                                        );
                                        return Box::new(
                                            web3::futures::future::ok::<(), _>(
                                                (),
                                            ),
                                        );
                                    }
                                }
                                return Box::new(web3::futures::future::err(e));
                            })
                            .map_err(|e| {
                                warn!(
                                    "Failed to send transaction. Error {}",
                                    e
                                );
                                error::Error::from(e)
                            })
                            .wait()?;

                        Ok(())
                    }
                    ConcernKey::KeyPair(_) => Err(Error::from(format!(
                        "Local signing not suported for worker"
                    ))),
                }
            })?
    }

    // Call when worker has been retired
    fn transfer_worker_funds(
        &self,
        web3: &web3::Web3<GenericTransport>,
        owner_address: &Address,
    ) -> Result<()> {
        trace!("Transfering worker funds back...");
        match &self.key {
            ConcernKey::UserAddress(address) => {
                let gas_price = {
                    let gp = web3.eth().gas_price().wait()?;
                    U256::from(2).checked_mul(gp).unwrap()
                };

                let (value, gas_limit) = {
                    let balance = web3.eth().balance(*address, None).wait()?;

                    let call_request = web3::types::CallRequest {
                        from: Some(*address),
                        to: *owner_address,
                        gas_price: Some(gas_price),
                        gas: None,
                        value: None,
                        data: None,
                    };

                    let gas_estimate =
                        web3.eth().estimate_gas(call_request, None).wait()?;

                    if balance > gas_estimate * gas_price {
                        (balance - gas_estimate * gas_price, gas_estimate)
                    } else {
                        trace!("Worker has insuficient funds to transfer back to owner");
                        return Ok(());
                    }
                };

                let tx_request = web3::types::TransactionRequest {
                    from: *address,
                    to: Some(*owner_address),
                    gas_price: Some(gas_price),
                    gas: Some(gas_limit),
                    value: Some(value),
                    data: None,
                    condition: None,
                    nonce: None,
                };

                trace!("Sending unsigned transaction, transfering {} from worker to owner", value);
                web3.eth()
                    .send_transaction(tx_request)
                    .map(|hash| {
                        info!("Transaction sent with hash: {:?}", hash);
                    })
                    .map_err(|e| {
                        warn!("Failed to send transaction. Error {}", e);
                        error::Error::from(e)
                    })
                    .wait()?;

                Ok(())
            }
            ConcernKey::KeyPair(_) => Err(Error::from(format!(
                "Local signing not suported for worker"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
