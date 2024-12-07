use std::fmt::Display;

use anyhow::{anyhow, Error, Result};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Clone, Eq)]
pub struct Hash {
    pub bytes: [u8; 32],
}
pub type UserId = Hash;
pub type GroupId = Hash;
pub type ObjectId = Hash;
pub type Content = Vec<u8>;
pub type UUID = [u8; 16];

impl TryFrom<Vec<u8>> for Hash {
    type Error = Error;

    fn try_from(bytes: Vec<u8>) -> Result<Self> {
        Ok(bytes[..].try_into()?)
    }
}
impl TryFrom<&[u8]> for Hash {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        Ok(Hash {
            bytes: bytes[..32].try_into()?,
        })
    }
}
impl TryFrom<&[u8; 32]> for Hash {
    type Error = Error;

    fn try_from(bytes: &[u8; 32]) -> Result<Self> {
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
impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(bs58::encode(&self.bytes).into_string().as_str())
    }
}
impl TryFrom<&String> for Hash {
    type Error = Error;
    fn try_from(value: &String) -> Result<Self> {
        bs58::decode(value).into_vec()?.as_slice().try_into()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupAccessToken {
    pub binding: GroupAccessBinding,
    pub group_owner_signature: Signature,
}

pub type UnixTimestamp = u64;

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupAccessBinding {
    pub group: GroupId,
    pub recipient: UserId,
    pub revocation_date: UnixTimestamp,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserGroup {
    pub definition: GroupDefinition,
    pub signature: Signature,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupDefinition {
    pub id: GroupId,
    pub owner: UserId,
    pub parent: GroupId,
    pub parent_membership_proof: GroupAccessToken,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Signature {
    Ed25519(SignatureEd25519),
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SignatureBytes {
    #[serde_as(as = "serde_with::Bytes")]
    pub bytes: [u8; 64],
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignatureEd25519 {
    pub verifying_key: [u8; 32],
    pub signature: SignatureBytes,
}

#[allow(unused)]
pub const TYPED_OBJECT_ID: UUID = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq)]
pub struct TypedObject {
    pub uuid: UUID,
    pub data: Vec<u8>,
}
impl TryFrom<Vec<u8>> for TypedObject {
    type Error = Error;
    fn try_from(value: Vec<u8>) -> std::result::Result<Self, Self::Error> {
        bincode::deserialize::<TypedObject>(&value).map_err(|e| anyhow!(e))
    }
}

#[allow(unused)]
pub const SIGNED_OBJECT_ID: UUID = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedObject {
    pub object: TypedObject,
    pub signature: Signature,
}

#[allow(unused)]
pub const GROUP_OBJECT_ID: UUID = [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupObject {
    pub group: GroupId,
    pub object: SignedObject,
}

#[allow(unused)]
pub const PLAIN_FILE_OBJECT_ID: UUID = [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlainFileObject {
    pub name: String,
    pub content: Content,
}
impl From<PlainFileObject> for TypedObject {
    fn from(obj: PlainFileObject) -> Self {
        TypedObject {
            uuid: PLAIN_FILE_OBJECT_ID,
            data: bincode::serialize(&obj).unwrap(),
        }
    }
}
impl TryFrom<&TypedObject> for PlainFileObject {
    type Error = Error;
    fn try_from(value: &TypedObject) -> Result<Self> {
        bincode::deserialize::<PlainFileObject>(&(value.data)).map_err(|e| anyhow!(e))
    }
}

#[allow(unused)]
pub const EMPTY_OBJECT_ID: UUID = [6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmptyObject {}
impl TypedObject {
    pub fn empty() -> Self {
        TypedObject {
            uuid: EMPTY_OBJECT_ID,
            data: bincode::serialize(&EmptyObject {}).unwrap(),
        }
    }
}

pub const QUERY_OBJECT_ID: UUID = [16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryObject {
    pub query_object: TypedObject,
}
impl From<QueryObject> for TypedObject {
    fn from(obj: QueryObject) -> Self {
        TypedObject {
            uuid: QUERY_OBJECT_ID,
            data: bincode::serialize(&obj).unwrap(),
        }
    }
}
impl TryFrom<&TypedObject> for QueryObject {
    type Error = Error;
    fn try_from(value: &TypedObject) -> Result<Self> {
        bincode::deserialize::<QueryObject>(&(value.data)).map_err(|e| anyhow!(e))
    }
}

#[allow(unused)]
pub const SIMPLE_ID_QUERY_ID: UUID = [8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimpleIDQuery {
    pub id: ObjectId,
}
impl From<SimpleIDQuery> for TypedObject {
    fn from(obj: SimpleIDQuery) -> Self {
        TypedObject {
            uuid: SIMPLE_ID_QUERY_ID,
            data: bincode::serialize(&obj).unwrap(),
        }
    }
}
impl From<SimpleIDQuery> for QueryObject {
    fn from(obj: SimpleIDQuery) -> Self {
        QueryObject {
            query_object: obj.into(),
        }
    }
}
impl TryFrom<&TypedObject> for SimpleIDQuery {
    type Error = Error;
    fn try_from(value: &TypedObject) -> Result<Self> {
        bincode::deserialize::<SimpleIDQuery>(&(value.data)).map_err(|e| anyhow!(e))
    }
}

#[allow(unused)]
pub const RESULT_OBJECT_ID: UUID = [9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResultObject {
    pub result: Result<(), ()>,
}
impl From<ResultObject> for TypedObject {
    fn from(obj: ResultObject) -> Self {
        TypedObject {
            uuid: RESULT_OBJECT_ID,
            data: bincode::serialize(&obj).unwrap(),
        }
    }
}
impl TryFrom<&TypedObject> for ResultObject {
    type Error = Error;
    fn try_from(value: &TypedObject) -> Result<Self> {
        bincode::deserialize::<ResultObject>(&(value.data)).map_err(|e| anyhow!(e))
    }
}
