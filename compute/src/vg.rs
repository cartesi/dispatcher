use super::configuration::{Concern, Configuration};
use super::dispatcher::{
    AddressField, Bytes32Field, FieldType, String32Field, U256Array5, U256Field,
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
use super::{Partition, Role};

pub struct VG();

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// these two structs and the From trait below shuld be
// obtained from a simple derive
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
#[derive(Serialize, Deserialize)]
struct VGCtxParsed(
    AddressField,  // challenger
    AddressField,  // claimer
    AddressField,  // machine
    Bytes32Field,  // initialHash
    Bytes32Field,  // claimedFinalHash
    Bytes32Field,  // hashBeforeDivergence
    Bytes32Field,  // hashAfterDivergence
    String32Field, // currentState
    U256Array5,    // uint values: roundDuration
                   //              finalTime
                   //              timeOfLastMove
                   //              mmInstance
                   //              partitionInstance
);

#[derive(Debug)]
struct VGCtx {
    challenger: Address,
    claimer: Address,
    machine: Address,
    initial_hash: H256,
    claimer_final_hash: H256,
    hash_before_divergence: H256,
    hash_after_divergence: H256,
    current_state: String,
    round_duration: U256,
    final_time: U256,
    time_of_last_move: U256,
    mm_instance: U256,
    partition_instance: U256,
}

impl From<VGCtxParsed> for VGCtx {
    fn from(parsed: VGCtxParsed) -> VGCtx {
        VGCtx {
            challenger: parsed.0.value,
            claimer: parsed.1.value,
            machine: parsed.2.value,
            initial_hash: parsed.3.value,
            claimer_final_hash: parsed.4.value,
            hash_before_divergence: parsed.5.value,
            hash_after_divergence: parsed.6.value,
            current_state: parsed.7.value,
            round_duration: parsed.8.value[0],
            final_time: parsed.8.value[1],
            time_of_last_move: parsed.8.value[2],
            mm_instance: parsed.8.value[3],
            partition_instance: parsed.8.value[4],
        }
    }
}

impl DApp<()> for VG {
    fn react(
        instance: &state::Instance,
        archive: &Archive,
        _: &(),
    ) -> Result<Reaction> {
        let parsed: VGCtxParsed = serde_json::from_str(&instance.json_data)
            .chain_err(|| {
                format!(
                    "Could not parse vg instance json_data: {}",
                    &instance.json_data
                )
            })?;
        let ctx: VGCtx = parsed.into();
        trace!("Context for vg (index {}) {:?}", instance.index, ctx);

        // should not happen as it indicates an innactive instance,
        // but it is possible that the blockchain state changed between queries
        match ctx.current_state.as_ref() {
            "FinishedClaimerWon" | "FinishedChallengerWon" => {
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
                "WaitPartition" => {
                    // pass control to the partition dapp
                    let partition_instance =
                        instance.sub_instances.get(0).ok_or(Error::from(
                            ErrorKind::InvalidContractState(format!(
                                "There is no partition instance {}",
                                ctx.current_state
                            )),
                        ))?;
                    return Partition::react(partition_instance, archive, &());
                }
                "WaitMemoryProveValues" => {
                    return Ok(Reaction::Idle); // does not concern claimer
                }
                _ => {
                    return Err(Error::from(ErrorKind::InvalidContractState(
                        format!("Unknown current state {}", ctx.current_state),
                    )));
                }
            },
            Role::Challenger => match ctx.current_state.as_ref() {
                "WaitPartition" => {
                    // pass control to the partition dapp
                    let partition_instance =
                        instance.sub_instances.get(0).ok_or(Error::from(
                            ErrorKind::InvalidContractState(format!(
                                "There is no partition instance {}",
                                ctx.current_state
                            )),
                        ))?;
                    return Partition::react(partition_instance, archive, &());
                }
                "WaitMemoryProveValues" => unimplemented!("w9sdf982"),
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
