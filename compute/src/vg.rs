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
use super::Role;

pub struct VG();

impl DApp<()> for VG {
    fn react(
        instance: &state::Instance,
        archive: &Archive,
        _: &(),
    ) -> Result<Reaction> {
        return Ok(Reaction::Idle);
    }
}
