use super::build_machine_id;
use super::configuration::{Concern, Configuration};
use super::dispatcher::{
    AddressField, BoolArray, Bytes32Array, Bytes32Field, FieldType,
    String32Field, U256Array, U256Array5, U256Field,
};
use super::dispatcher::{Archive, DApp, Reaction, SampleRequest};
use super::error::Result;
use super::error::*;
use super::ethabi::Token;
use super::ethereum_types::{Address, H256, U256};
use super::serde::de::Error as SerdeError;
use super::serde::{Deserialize, Deserializer, Serializer};
use super::serde_json::Value;
use super::state::Instance;
use super::transaction::TransactionRequest;
use super::Role;

use std::collections::HashSet;

pub struct Partition();

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// these two structs and the From trait below shuld be
// obtained from a simple derive
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
#[derive(Serialize, Deserialize)]
struct PartitionCtxParsed(
    AddressField,  // challenger
    AddressField,  // claimer
    U256Array,     // queryArray
    BoolArray,     // submittedArray
    Bytes32Array,  // hashArray
    String32Field, // currentState
    U256Array5,    // uint values: finalTime
                   // querySize
                   // timeOfLastMove
                   // roundDuration
                   // divergenceTime
);

#[derive(Debug)]
struct PartitionCtx {
    challenger: Address,
    claimer: Address,
    query_array: Vec<U256>,
    submitted_array: Vec<bool>,
    hash_array: Vec<H256>,
    current_state: String,
    final_time: U256,
    query_size: U256,
    time_of_last_move: U256,
    round_duration: U256,
    divergence_time: U256,
}

impl From<PartitionCtxParsed> for PartitionCtx {
    fn from(parsed: PartitionCtxParsed) -> PartitionCtx {
        PartitionCtx {
            challenger: parsed.0.value,
            claimer: parsed.1.value,
            query_array: parsed.2.value,
            submitted_array: parsed.3.value,
            hash_array: parsed.4.value,
            current_state: parsed.5.value,
            final_time: parsed.6.value[0],
            query_size: parsed.6.value[1],
            time_of_last_move: parsed.6.value[2],
            round_duration: parsed.6.value[3],
            divergence_time: parsed.6.value[4],
        }
    }
}

impl DApp<()> for Partition {
    fn react(
        instance: &state::Instance,
        archive: &Archive,
        _: &(),
    ) -> Result<Reaction> {
        let parsed: PartitionCtxParsed =
            serde_json::from_str(&instance.json_data).chain_err(|| {
                format!(
                    "Could not parse partition instance json_data: {}",
                    &instance.json_data
                )
            })?;
        let ctx: PartitionCtx = parsed.into();
        trace!("Context for parition {:?}", ctx);

        // should not happen as it indicates an innactive instance,
        // but it is possible that the blockchain state changed between queries
        match ctx.current_state.as_ref() {
            "ChallengerWon" | "ClaimerWon" | "DivergenceFound" => {
                return Ok(Reaction::Idle);
            }
            _ => {}
        };

        // if we reach this code, the instance is active
        let role = match instance.concern.user_address {
            cl if (cl == ctx.claimer) => Role::Claimer,
            ch if (ch == ctx.challenger) => Role::Challenger,
            _ => {
                return Err(Error::from(ErrorKind::InvalidContractState(
                    String::from("User is neither claimer nor challenger"),
                )));
            }
        };
        trace!("Role played (index {}) is: {:?}", instance.index, role);

        match role {
            Role::Claimer => match ctx.current_state.as_ref() {
                "WaitingQuery" => {
                    return Ok(Reaction::Idle); // does not concern claimer
                }
                "WaitingHashes" => {
                    // machine id
                    let id = build_machine_id(
                        instance.index,
                        &instance.concern.contract_address,
                    );
                    trace!("Calculating queried hashes of machine {}", id);
                    let mut hashes = Vec::new();
                    // have we sampled this machine yet?
                    if let Some(samples) = archive.get(&id) {
                        // take the run samples (not the step samples)
                        let run_samples = &samples.0;
                        for i in 0..ctx.query_size.as_usize() {
                            // get the i'th time in query array
                            let time = &ctx.query_array.get(i).ok_or(
                                Error::from(ErrorKind::InvalidContractState(
                                    String::from(
                                        "could not find element in query array",
                                    ),
                                )),
                            )?;
                            // have we sampled that specific time?
                            match run_samples.get(time) {
                                Some(hash) => hashes.push(hash),
                                None => {
                                    // some hash not calculated yet, request all
                                    let sample_points: HashSet<U256> = ctx
                                        .query_array
                                        .clone()
                                        .into_iter()
                                        .collect();
                                    return Ok(Reaction::Request((
                                        id,
                                        sample_points,
                                    )));
                                }
                            }
                        }
                        // submit the required hashes
                        let request = TransactionRequest {
                            concern: instance.concern.clone(),
                            value: U256::from(0),
                            function: "replyQuery".into(),
                            // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                            // improve these types by letting the
                            // dapp submit ethereum_types and convert
                            // them inside the transaction manager
                            // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                            data: vec![
                                Token::Uint(instance.index),
                                Token::Array(
                                    ctx.query_array
                                        .clone()
                                        .iter_mut()
                                        .map(|q: &mut U256| -> _ {
                                            Token::Uint(q.clone())
                                        })
                                        .collect(),
                                ),
                                Token::Array(
                                    hashes
                                        .into_iter()
                                        .map(|h| -> _ {
                                            Token::FixedBytes(
                                                h.clone().to_vec(),
                                            )
                                        })
                                        .collect(),
                                ),
                            ],
                            strategy: transaction::Strategy::Simplest,
                        };
                        return Ok(Reaction::Transaction(request));
                    }
                    // machine not queried yet (power outage?), request all
                    let sample_points: HashSet<U256> =
                        ctx.query_array.clone().into_iter().collect();
                    return Ok(Reaction::Request((id, sample_points)));
                }
                _ => {
                    return Err(Error::from(ErrorKind::InvalidContractState(
                        format!("Unknown current state {}", ctx.current_state),
                    )));
                }
            },
            Role::Challenger => match ctx.current_state.as_ref() {
                "WaitingQuery" => {
                    // machine id
                    let id = build_machine_id(
                        instance.index,
                        &instance.concern.contract_address,
                    );
                    trace!("Calculating posted hashes of machine {}", id);
                    // have we sampled this machine yet?
                    if let Some(samples) = archive.get(&id) {
                        // take the run samples (not the step samples)
                        let run_samples = &samples.0;
                        for i in 0..(ctx.query_size.as_usize() - 1) {
                            // get the i'th time in query array
                            let time =
                                ctx.query_array.get(i).ok_or(Error::from(
                                    ErrorKind::InvalidContractState(format!(
                                    "could not find element {} in query array",
                                    i
                                )),
                                ))?;
                            let next_time = ctx.query_array.get(i + 1).ok_or(
                                Error::from(ErrorKind::InvalidContractState(
                                    format!(
                                    "could not find element {} in query array",
                                    i
                                ),
                                )),
                            )?;
                            // get the i'th hash in hash array
                            let claimed_hash =
                                &ctx.hash_array.get(i).ok_or(Error::from(
                                    ErrorKind::InvalidContractState(format!(
                                    "could not find element {} in hash array",
                                    i
                                )),
                                ))?;
                            // have we sampled that specific time?
                            let hash = match run_samples.get(time) {
                                Some(hash) => hash,
                                None => {
                                    // some hash not calculated yet, request all
                                    let sample_points: HashSet<U256> = ctx
                                        .query_array
                                        .clone()
                                        .into_iter()
                                        .collect();
                                    return Ok(Reaction::Request((
                                        id,
                                        sample_points,
                                    )));
                                }
                            };

                            if hash != *claimed_hash {
                                // submit the relevant query
                                let request = TransactionRequest {
                                    concern: instance.concern.clone(),
                                    value: U256::from(0),
                                    function: "makeQuery".into(),
                                    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                                    // improve these types by letting the
                                    // dapp submit ethereum_types and convert
                                    // them inside the transaction manager
                                    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                                    data: vec![
                                        Token::Uint(instance.index),
                                        Token::Uint(U256::from(i - 1)),
                                        Token::Uint(*time),
                                        Token::Uint(*next_time),
                                    ],
                                    strategy: transaction::Strategy::Simplest,
                                };
                                return Ok(Reaction::Transaction(request));
                            }
                        }
                        // no disagreement found. important bug!!!!
                        error!(
                            "bug found, no disagreement in dispute {:?}!!!",
                            instance
                        );
                    }
                    // machine not queried yet (power outage?), request all
                    let sample_points: HashSet<U256> =
                        ctx.query_array.clone().into_iter().collect();
                    return Ok(Reaction::Request((id, sample_points)));
                }
                "WaitingHashes" => {
                    return Ok(Reaction::Idle); // does not concern challenger
                }
                _ => {
                    return Err(Error::from(ErrorKind::InvalidContractState(
                        format!("Unknown current state {}", ctx.current_state),
                    )));
                }
            },
        }

        return Ok(Reaction::Idle);
    }
}