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
extern crate serde;
extern crate serde_json;
extern crate state;

use configuration::{Concern, Configuration};
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

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// Do proper serialization/deserialization of these types and
// share the implementation with that of get_instance in state manager.
// Also make sure that the FieldType match the expected one.
// Probably need a custom Deserialize implementation, see:
// https://github.com/serde-rs/serde/issues/760
//
// This implementation should also be moved an outside library to make
// it very easy for new DApps to specify the expected struct
//
// We should also check if the name of the arguments match,
// like "_challenger".
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
#[derive(Serialize, Deserialize)]
enum FieldType {
    #[serde(rename = "address")]
    AddressType,
    #[serde(rename = "uint256")]
    U256Type,
    #[serde(rename = "uint8")]
    U8Type,
    #[serde(rename = "bytes32")]
    Bytes32Type,
}

#[derive(Serialize, Deserialize)]
struct AddressField {
    name: String,
    #[serde(rename = "type")]
    ty: FieldType,
    value: Address,
}

#[derive(Serialize, Deserialize)]
struct U256Field {
    name: String,
    #[serde(rename = "type")]
    ty: FieldType,
    value: U256,
}

#[derive(Serialize, Deserialize)]
struct Bytes32Field {
    name: String,
    #[serde(rename = "type")]
    ty: FieldType,
    value: H256,
}

fn u8_from_hex<'de, D>(deserializer: D) -> std::result::Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    // do better hex decoding than this
    u8::from_str_radix(&s[2..], 16).map_err(D::Error::custom)
}

#[derive(Serialize, Deserialize)]
struct U8Field {
    name: String,
    #[serde(rename = "type")]
    ty: FieldType,
    #[serde(deserialize_with = "u8_from_hex")]
    value: u8,
}

#[derive(Serialize, Deserialize)]
struct ComputeCtxParsed(
    AddressField, // challenger
    AddressField, // claimer
    U256Field,    // roundDuration
    U256Field,    // timeOfLastMove
    AddressField, // machine
    Bytes32Field, // initialHash
    U256Field,    // finalTime
    Bytes32Field, // claimedFinalHash
    U8Field,      // currentState
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
    current_state: u8,
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
            match serde_json::from_str(&instance.json_data) {
                Ok(s) => s,
                Err(e) => {
                    return Err(Error::from(ErrorKind::InvalidContractState(
                        String::from("User is neither claimer nor challenger"),
                    )));
                }
            };
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
        // but could have just changed
        match ctx.current_state {
            // correspond respectively to:
            // ClaimerMissedDeadline, ChallengerWon, ClaimerWon, ConsensusResult
            2 | 4 | 5 | 6 => {
                return Ok(Reaction::Idle);
            }
            _ => {}
        };

        // if not innactive
        match role {
            Role::Claimer => match ctx.current_state {
                // WaitingConfirmation
                1 => {
                    // does not concern claimer
                    return Ok(Reaction::Idle);
                }
                // WaitingClaim
                0 => {}
                // WaitingChallenge
                3 => {
                    // here goes the verification game
                }
                _ => unreachable!(),
            },
            Role::Challenger => match ctx.current_state {
                // WaitingConfirmation
                1 => {
                    // here we check the result and potentialy raise challenge
                }
                // WaitingClaim
                0 => {
                    // does not concern challenger
                    return Ok(Reaction::Idle);
                }
                3 => {
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
