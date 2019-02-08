use super::build_machine_id;
use super::configuration::{Concern, Configuration};
use super::dispatcher::{
    AddressField, Bytes32Field, FieldType, String32Field, U256Field,
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
use super::transaction;
use super::transaction::TransactionRequest;
use super::{Role, VG};
use std::collections::{HashMap, HashSet};

use std::time::{SystemTime, UNIX_EPOCH};
use time::Duration;

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
            serde_json::from_str(&instance.json_data).chain_err(|| {
                format!(
                    "Could not parse compute instance json_data: {}",
                    &instance.json_data
                )
            })?;
        let ctx: ComputeCtx = parsed.into();
        trace!("Context for compute (index {}) {:?}", instance.index, ctx);

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
                "WaitingConfirmation" => {
                    return win_by_deadline_or_idle(
                        &instance.concern,
                        instance.index,
                        &ctx,
                    );
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
                        // take the run samples (not the step samples)
                        let run_samples = &samples.run;
                        // have we sampled the final time?
                        if let Some(hash) = run_samples.get(&ctx.final_time) {
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
                        [U256::from(0), ctx.final_time]
                            .iter()
                            .cloned()
                            .collect();
                    return Ok(Reaction::Request(SampleRequest {
                        id: id,
                        times: sample_points,
                    }));
                }
                "WaitingChallenge" => {
                    // pass control to the verification game dapp
                    let vg_instance = instance.sub_instances.get(0).ok_or(
                        Error::from(ErrorKind::InvalidContractState(format!(
                            "There is no vg instance {}",
                            ctx.current_state
                        ))),
                    )?;
                    return VG::react(vg_instance, archive, &());
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
                        let run_samples = &samples.run;
                        // have we sampled the final time?
                        if let Some(hash) = run_samples.get(&ctx.final_time) {
                            if hash == &ctx.claimed_final_hash {
                                info!(
                                    "Confirming final hash {:?} for {}",
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
                                    "Disputing final hash {:?} != {} for {}",
                                    hash, ctx.claimed_final_hash, id
                                );
                                let request = TransactionRequest {
                                    concern: instance.concern.clone(),
                                    value: U256::from(0),
                                    function: "challenge".into(),
                                    data: vec![Token::Uint(instance.index)],
                                    strategy: transaction::Strategy::Simplest,
                                };

                                return Ok(Reaction::Transaction(request));
                            }
                        }
                    };
                    // final hash has not been calculated yet, request it
                    let sample_points: HashSet<U256> =
                        [U256::from(0), ctx.final_time]
                            .iter()
                            .cloned()
                            .collect();
                    return Ok(Reaction::Request(SampleRequest {
                        id: id,
                        times: sample_points,
                    }));
                }
                "WaitingClaim" => {
                    return win_by_deadline_or_idle(
                        &instance.concern,
                        instance.index,
                        &ctx,
                    );
                }
                "WaitingChallenge" => {
                    // pass control to the verification game dapp
                    let vg_instance = instance.sub_instances.get(0).ok_or(
                        Error::from(ErrorKind::InvalidContractState(format!(
                            "There is no vg instance {}",
                            ctx.current_state
                        ))),
                    )?;
                    return VG::react(vg_instance, archive, &());
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

fn win_by_deadline_or_idle(
    concern: &Concern,
    index: U256,
    ctx: &ComputeCtx,
) -> Result<Reaction> {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .chain_err(|| "System time before UNIX_EPOCH")?
        .as_secs();

    // if other party missed the deadline
    if (current_time
        > ctx.time_of_last_move.as_u64() + ctx.round_duration.as_u64())
    {
        error!("AAA");
        let request = TransactionRequest {
            concern: concern.clone(),
            value: U256::from(0),
            function: "claimVictoryByTime".into(),
            data: vec![Token::Uint(index)],
            strategy: transaction::Strategy::Simplest,
        };
        return Ok(Reaction::Transaction(request));
    } else {
        // if not, then wait
        return Ok(Reaction::Idle);
    }
}
