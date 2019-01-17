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
extern crate hex;
extern crate serde;
extern crate serde_json;
extern crate state;

use configuration::{Concern, Configuration};
use dispatcher::{
    AddressField, Bytes32Field, FieldType, String32Field, U256Field,
};
use dispatcher::{Archive, DApp, MachineArchive, MachinePoint, Reaction};
use error::Result;
use error::*;
use ethabi::Token;
use ethereum_types::{Address, H256, U256};
use serde::de::Error as SerdeError;
use serde::{Deserialize, Deserializer, Serializer};
use serde_json::Value;
use state::Instance;

pub struct VG();

impl VG {
    pub fn new() -> Self {
        VG()
    }
}

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

#[derive(Debug)]
enum Role {
    Claimer,
    Challenger,
}

impl DApp for VG {
    fn react(
        &self,
        instance: &state::Instance,
        archive: &Archive,
    ) -> Result<Reaction> {
        let parsed: ComputeCtxParsed =
            serde_json::from_str(&instance.json_data)
                .chain_err(|| "Could not parse instance json_data")?;
        let ctx: ComputeCtx = parsed.into();
        trace!("Context for compute {:?}", ctx);

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

        // if not innactive
        match role {
            Role::Claimer => match ctx.current_state.as_ref() {
                "WaitingConfirmation" => {
                    // does not concern claimer
                    return Ok(Reaction::Idle);
                }
                "WaitingClaim" => {}
                "WaitingChallenge" => {
                    // here goes the verification game
                }
                _ => unreachable!(),
            },
            Role::Challenger => match ctx.current_state.as_ref() {
                "WaitingConfirmation" => {
                    // here we check the result and potentialy raise challenge
                }
                "WaitingClaim" => {
                    // does not concern challenger
                    return Ok(Reaction::Idle);
                }
                "WaitingChallenge" => {
                    // here goes the verification game
                }
                _ => unreachable!(),
            },
        }

        //let machine_archive = MachineArchive {
        //    instance.
        //}
        let request_archive = vec![];
        //request_archive.push()
        return Ok(Reaction::MachineRequest(Archive {
            machines: request_archive,
        }));
    }
}
