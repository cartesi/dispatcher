use super::emulator::types;
use super::error::*;
use super::ethabi::Token;
use super::ethereum_types::{Address, H256, U256};
use super::serde::de::Error as SerdeError;
use super::serde::{Deserialize, Deserializer, Serializer};
use super::serde_json::Value;
use super::state::Instance;
use super::transaction::TransactionRequest;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct Proof {
    address: u64,
    subtree: H256,
    siblings: Vec<H256>,
}

pub type SampleRun = HashMap<U256, H256>;

pub type SampleStep = HashMap<U256, Proof>;

pub type Archive = HashMap<String, (SampleRun, SampleStep)>;

pub type SampleRequest = (String, HashSet<U256>);

pub type StepRequest = (String, U256);

#[derive(Debug)]
pub enum Reaction {
    Request(SampleRequest),
    Step(StepRequest),
    Transaction(TransactionRequest),
    Idle,
}

pub fn add_run(archive: &mut Archive, id: String, time: U256, hash: H256) {
    let mut samples = archive
        .entry(id.clone())
        .or_insert((SampleRun::new(), SampleStep::new()));
    //samples.0.insert(time, hash);
    if let Some(s) = samples.0.insert(time, hash) {
        warn!("Machine {} at time {} recomputed", id, time);
    }
}

pub fn add_step(archive: &mut Archive, id: String, time: U256, proof: Proof) {
    let mut samples = archive
        .entry(id.clone())
        .or_insert((SampleRun::new(), SampleStep::new()));
    //samples.0.insert(time, hash);
    if let Some(s) = samples.1.insert(time, proof) {
        warn!("Machine {} at time {} recomputed", id, time);
    }
}

pub trait DApp<T> {
    fn react(&state::Instance, &Archive, &T) -> Result<Reaction>;
}

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// Do proper serialization/deserialization of these types and
// share the implementation with that of get_instance in state manager.
// Also make sure that the FieldType match the expected one.
// Probably need a custom Deserialize implementation, see:
// https://github.com/serde-rs/serde/issues/760
//
// We should also check if the name of the arguments match,
// like "_challenger".
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
#[derive(Serialize, Deserialize)]
pub enum FieldType {
    #[serde(rename = "address")]
    AddressType,
    #[serde(rename = "uint256")]
    U256Type,
    #[serde(rename = "uint8")]
    U8Type,
    #[serde(rename = "bytes32")]
    Bytes32Type,
    #[serde(rename = "uint256[5]")]
    U256Array5Type,
    #[serde(rename = "uint256[]")]
    U256ArrayType,
    #[serde(rename = "bool[]")]
    BoolArrayType,
    #[serde(rename = "bytes32[]")]
    Bytes32ArrayType,
}

#[derive(Serialize, Deserialize)]
pub struct AddressField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    pub value: Address,
}

#[derive(Serialize, Deserialize)]
pub struct U256Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    pub value: U256,
}

#[derive(Serialize, Deserialize)]
pub struct U256Array {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    pub value: Vec<U256>,
}

#[derive(Serialize, Deserialize)]
pub struct U256Array5 {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    pub value: [U256; 5],
}

#[derive(Serialize, Deserialize)]
pub struct Bytes32Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    pub value: H256,
}

#[derive(Serialize, Deserialize)]
pub struct Bytes32Array {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    pub value: Vec<H256>,
}

#[derive(Serialize, Deserialize)]
pub struct BoolArray {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    pub value: Vec<bool>,
}

fn string_from_hex<'de, D>(
    deserializer: D,
) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    // do better hex decoding than this
    hex::decode(&s[2..])
        .map_err(D::Error::custom)
        .and_then(|vec_u8| {
            let removed_trailing_zeros =
                vec_u8.iter().take_while(|&n| *n != 0).map(|&n| n).collect();
            String::from_utf8(removed_trailing_zeros).map_err(D::Error::custom)
        })
}

#[derive(Serialize, Deserialize)]
pub struct String32Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    #[serde(deserialize_with = "string_from_hex")]
    pub value: String,
}
