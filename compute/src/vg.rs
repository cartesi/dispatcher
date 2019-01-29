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
use super::Role;

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
    [U256Field; 5], // uint values: roundDuration
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
            round_duration: parsed.8[0].value,
            final_time: parsed.8[1].value,
            time_of_last_move: parsed.8[2].value,
            mm_instance: parsed.8[3].value,
            partition_instance: parsed.8[4].value,
        }
    }
}

impl DApp<()> for VG {
    fn react(
        instance: &state::Instance,
        archive: &Archive,
        _: &(),
    ) -> Result<Reaction> {
        warn!("Instance {} reached vg", instance.index);
        return Ok(Reaction::Idle);
    }
}
