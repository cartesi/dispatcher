use super::configuration::{Concern, Configuration};
use super::dispatcher::{
    AddressField, Bytes32Field, FieldType, String32Field, U256Field,
};
use super::dispatcher::{Archive, DApp, Reaction, SampleRequest, Samples};
use super::error::Result;
use super::error::*;
use super::ethabi::Token;
use super::ethereum_types::{Address, H256, U256};
use super::serde::de::Error as SerdeError;
use super::serde::{Deserialize, Deserializer, Serializer};
use super::serde_json::Value;
use super::state::Instance;
use super::transaction;
use super::transaction::TransactionRequest;
use super::{Role, VG};
use std::collections::{HashMap, HashSet};

pub struct Compute();

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// these two structs and the From trait below shuld be
// obtained from a simple derive
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
#[derive(Serialize, Deserialize)]
struct ComputeCtxParsed(
    AddressField,  // challenger
    AddressField,  // claimer
    U256Field,     // roundDuration
    U256Field,     // timeOfLastMove
    AddressField,  // machine
    Bytes32Field,  // initialHash
    U256Field,     // finalTime
    Bytes32Field,  // claimedFinalHash
    String32Field, // currentState
);

#[derive(Debug)]
struct ComputeCtx {
    challenger: Address,
    claimer: Address,
    round_duration: U256,
    time_of_last_move: U256,
    machine: Address,
    initial_hash: H256,
    final_time: U256,
    claimed_final_hash: H256,
    current_state: String,
}

impl From<ComputeCtxParsed> for ComputeCtx {
    fn from(parsed: ComputeCtxParsed) -> ComputeCtx {
        ComputeCtx {
            challenger: parsed.0.value,
            claimer: parsed.1.value,
            round_duration: parsed.2.value,
            time_of_last_move: parsed.3.value,
            machine: parsed.4.value,
            initial_hash: parsed.5.value,
            final_time: parsed.6.value,
            claimed_final_hash: parsed.7.value,
            current_state: parsed.8.value,
        }
    }
}

impl DApp<()> for Compute {
    fn react(
        instance: &state::Instance,
        archive: &Archive,
        _: &(),
    ) -> Result<Reaction> {
        let parsed: ComputeCtxParsed =
            serde_json::from_str(&instance.json_data)
                .chain_err(|| "Could not parse instance json_data")?;
        let ctx: ComputeCtx = parsed.into();
        trace!("Context for compute {:?}", ctx);

        // should not happen as it indicates an innactive instance,
        // but it is possible that the blockchain state changed between queries
        match ctx.current_state.as_ref() {
            "ClaimerMissedDeadline"
            | "ChallengerWon"
            | "ClaimerWon"
            | "ConsensusResult" => {
                return Ok(Reaction::Idle);
            }
            _ => {}
        };

        // reaching here this instance is active
        let role = match instance.concern.user_address {
            cl if (cl == ctx.claimer) => Role::Claimer,
            ch if (ch == ctx.challenger) => Role::Challenger,
            _ => {
                return Err(Error::from(ErrorKind::InvalidContractState(
                    String::from("User is neither claimer nor challenger"),
                )));
            }
        };

        trace!("Role played is: {:?}", role);

        match role {
            Role::Claimer => match ctx.current_state.as_ref() {
                "WaitingConfirmation" => {
                    return Ok(Reaction::Idle); // does not concern claimer
                }
                "WaitingClaim" => {
                    // machine id
                    let id = build_machine_id(
                        instance.index,
                        &instance.concern.contract_address,
                    );
                    trace!("Calculating final hash of machine {}", id);
                    // have we sampled this machine yet?
                    if let Some(samples) = archive.get(&id) {
                        // have we sampled the final time?
                        if let Some(hash) = samples.get(&ctx.final_time) {
                            // then submit the final hash
                            let request = TransactionRequest {
                                concern: instance.concern.clone(),
                                value: U256::from(0),
                                function: "submitClaim".into(),
                                // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                                // improve these types by letting the
                                // dapp submit ethereum_types and convert
                                // them inside the transaction manager
                                // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                                data: vec![
                                    Token::Uint(instance.index),
                                    Token::FixedBytes(hash.0.to_vec()),
                                ],
                                strategy: transaction::Strategy::Simplest,
                            };
                            return Ok(Reaction::Transaction(request));
                        }
                    };
                    // final hash has not been calculated yet, request it
                    let sample_points: HashSet<U256> =
                        [ctx.final_time].iter().cloned().collect();
                    return Ok(Reaction::Request((id, sample_points)));
                }
                "WaitingChallenge" => {
                    // pass control to the verification game dapp
                    return VG::react(instance, archive, &());
                }
                _ => {
                    return Err(Error::from(ErrorKind::InvalidContractState(
                        format!("Unknown current state {}", ctx.current_state),
                    )));
                }
            },
            Role::Challenger => match ctx.current_state.as_ref() {
                "WaitingConfirmation" => {
                    // here goes the calculation of the final hash
                    // to check the claim and potentialy raise challenge
                    // machine id
                    let id = build_machine_id(
                        instance.index,
                        &instance.concern.contract_address,
                    );
                    trace!("Calculating final hash of machine {}", id);
                    // have we sampled this machine yet?
                    if let Some(samples) = archive.get(&id) {
                        // have we sampled the final time?
                        if let Some(hash) = samples.get(&ctx.final_time) {
                            if hash == &ctx.claimed_final_hash {
                                info!(
                                    "Confirming final hash {} for {}",
                                    hash, id
                                );
                                let request = TransactionRequest {
                                    concern: instance.concern.clone(),
                                    value: U256::from(0),
                                    function: "confirm".into(),
                                    data: vec![Token::Uint(instance.index)],
                                    strategy: transaction::Strategy::Simplest,
                                };
                                return Ok(Reaction::Transaction(request));
                            } else {
                                warn!(
                                    "Disputing final hash {} != {} for {}",
                                    hash, ctx.claimed_final_hash, id
                                );
                                let request = TransactionRequest {
                                    concern: instance.concern.clone(),
                                    value: U256::from(0),
                                    function: "challange".into(),
                                    data: vec![Token::Uint(instance.index)],
                                    strategy: transaction::Strategy::Simplest,
                                };

                                return Ok(Reaction::Transaction(request));
                            }
                        }
                    };
                    // final hash has not been calculated yet, request it
                    let sample_points: HashSet<U256> =
                        [ctx.final_time].iter().cloned().collect();
                    return Ok(Reaction::Request((id, sample_points)));
                }
                "WaitingClaim" => {
                    return Ok(Reaction::Idle); // does not concern challenger
                }
                "WaitingChallenge" => {
                    // pass control to the verification game dapp
                    return VG::react(instance, archive, &());
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

fn build_machine_id(index: U256, address: &Address) -> String {
    return format!("{:x}:{}", address, index);
}
