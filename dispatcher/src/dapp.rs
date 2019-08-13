// Note: This component currently has dependencies that are licensed under the GNU GPL, version 3, and so you should treat this component as a whole as being under the GPL version 3. But all Cartesi-written code in this component is licensed under the Apache License, version 2, or a compatible permissive license, and can be used independently under the Apache v2 license. After this component is rewritten, the entire component will be released under the Apache v2 license.

// Copyright 2019 Cartesi Pte. Ltd.

// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License. You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.




use super::error::*;
use super::ethereum_types::{Address, H256, U256};
use super::serde::de::Error as SerdeError;
use super::serde::{Deserialize, Deserializer};
use super::transaction::TransactionRequest;
use std::collections::HashMap;

/// Stores the hash of each time that has been calculated
pub type SampleRun = HashMap<U256, H256>;

/// The log of a given step, composed of various accesses (read/write)
pub type StepLog = Vec<emulator::Access>;

/// Stores step logs of each time for which it has been calculated
pub type SampleStep = HashMap<U256, StepLog>;

/// This is our collection of samples (both of running and step logs)
pub struct SamplePair {
    pub run: SampleRun,
    pub step: SampleStep,
}

/// The total archive, for each machine session
pub type Archive = HashMap<String, SamplePair>;

use emulator::SessionRunRequest;
use emulator::SessionStepRequest;

// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// In the future, there should be app emulator
// commands available here. Also, the enum for these
// commands should be inside the emulator crate
// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

/// A dapp can react in any of these ways:
/// . Request the machine to run and log the hashes to archive
/// . Request the machine to give one logged step and save it to archive
/// . Submit a transaction to the blockchain
/// . Idle and do nothing
#[derive(Debug)]
pub enum Reaction {
    Request(SessionRunRequest),
    Step(SessionStepRequest),
    Transaction(TransactionRequest),
    Idle,
}

pub trait DApp<T> {
    /// The function that makes a certain dapp react to the state of the
    /// instance
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
    #[serde(rename = "uint256[6]")]
    U256Array6Type,
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
pub struct U256Array6 {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: FieldType,
    pub value: [U256; 6],
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
