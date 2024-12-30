pub mod file;
pub mod group;
pub mod queries;
pub mod signed;

use std::fmt::Display;

use anyhow::{anyhow, Error, Result};
use libp2p::identity::PublicKey;
use libp2p::kad::RecordKey;
use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Clone, Eq)]
pub struct Hash {
    pub bytes: [u8; 32],
}

pub trait UUIDTyped {
    fn get_type_uuid(&self) -> Uuid;
}

impl<T> From<T> for TypedObject
where
    T: UUIDTyped + Serialize,
{
    fn from(value: T) -> Self {
        TypedObject {
            uuid: value.get_type_uuid(),
            data: bincode::serialize(&value).unwrap(),
        }
    }
}

impl TypedObject {
    pub fn try_from_typed<T>(value: &TypedObject) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        bincode::deserialize::<T>(&value.data).map_err(|e| anyhow!(e))
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", bs58::encode(self.bytes).into_string())
    }
}

impl TryFrom<Vec<u8>> for Hash {
    type Error = Error;

    fn try_from(bytes: Vec<u8>) -> Result<Self> {
        Ok(bytes[..].try_into()?)
    }
}
impl TryFrom<&[u8]> for Hash {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(anyhow!(
                "Hash has 32 bytes, tried to convert from {} bytes",
                bytes.len()
            ));
        }
        Ok(Hash {
            bytes: bytes[..32].try_into()?,
        })
    }
}
impl TryFrom<&[u8; 32]> for Hash {
    type Error = Error;

    fn try_from(bytes: &[u8; 32]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(anyhow!(
                "Hash has 32 bytes, tried to convert from {} bytes",
                bytes.len()
            ));
        }
        Ok(Hash {
            bytes: bytes[..32].try_into()?,
        })
    }
}
impl TryFrom<&TypedObject> for Hash {
    type Error = Error;

    fn try_from(value: &TypedObject) -> Result<Self> {
        blake3::hash(bincode::serialize(value)?.as_slice())
            .as_bytes()
            .try_into()
    }
}

impl TryFrom<&str> for Hash {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        bs58::decode(value).into_vec()?.as_slice().try_into()
    }
}
impl Into<libp2p::kad::RecordKey> for Hash {
    fn into(self) -> libp2p::kad::RecordKey {
        RecordKey::new(&self.bytes)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq)]
pub struct TypedObject {
    pub uuid: Uuid,
    pub data: Vec<u8>,
}
impl TypedObject {
    pub const UUID: Uuid = uuid!("0193a7be-425b-7158-8677-2dfdb28d3b00");
}
impl TypedObject {
    pub fn get_uuid(&self) -> Uuid {
        TypedObject::UUID
    }
}
impl TryFrom<&Vec<u8>> for TypedObject {
    type Error = Error;
    fn try_from(value: &Vec<u8>) -> std::result::Result<Self, Self::Error> {
        bincode::deserialize::<TypedObject>(value).map_err(|e| anyhow!(e))
    }
}
impl TryInto<Vec<u8>> for TypedObject {
    type Error = Error;
    fn try_into(self) -> std::result::Result<Vec<u8>, Self::Error> {
        bincode::serialize(&self).map_err(|e| anyhow!(e))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerializablePublicKey {
    pub key: Vec<u8>,
}
impl From<libp2p::identity::PublicKey> for SerializablePublicKey {
    fn from(value: libp2p::identity::PublicKey) -> Self {
        SerializablePublicKey {
            key: bincode::serialize(&value.encode_protobuf()).unwrap(),
        }
    }
}
impl TryInto<libp2p::identity::PublicKey> for SerializablePublicKey {
    type Error = Error;
    fn try_into(self) -> Result<libp2p::identity::PublicKey> {
        PublicKey::try_decode_protobuf(bincode::deserialize(&self.key)?).map_err(|e| anyhow!(e))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Signature {
    pub bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmptyObject {}
impl EmptyObject {
    pub const UUID: Uuid = uuid!("0193a7c0-5e33-7957-b34f-7ec0c4aa27f4");
}
impl UUIDTyped for EmptyObject {
    fn get_type_uuid(&self) -> Uuid {
        EmptyObject::UUID
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResultObject {
    pub result: Result<(), ()>,
}
impl ResultObject {
    pub const UUID: Uuid = uuid!("0193a7c0-be9b-72fa-b216-fb91814cba4f");
}
impl UUIDTyped for ResultObject {
    fn get_type_uuid(&self) -> Uuid {
        ResultObject::UUID
    }
}
