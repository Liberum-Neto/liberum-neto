use anyhow::{anyhow, Error, Result};
use libp2p;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Clone, Eq)]
pub(crate) struct Hash {
    pub bytes: [u8; 32],
}
pub(crate) type UserId = Hash;
pub(crate) type GroupId = Hash;
pub(crate) type ObjectId = Hash;
pub(crate) type Content = Vec<u8>;
pub(crate) type UUID = [u8; 16];

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

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct GroupAccessToken {
    pub binding: GroupAccessBinding,
    pub group_owner_signature: Signature,
}

pub(crate) type UnixTimestamp = u64;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct GroupAccessBinding {
    pub group: GroupId,
    pub recipient: UserId,
    pub revocation_date: UnixTimestamp,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct UserGroup {
    definition: GroupDefinition,
    signature: Signature,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct GroupDefinition {
    id: GroupId,
    owner: UserId,
    parent: GroupId,
    parent_membership_proof: GroupAccessToken,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum Signature {
    Ed25519(SignatureEd25519),
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub(crate) struct SignatureBytes {
    #[serde_as(as = "serde_with::Bytes")]
    bytes: [u8; 64],
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SignatureEd25519 {
    pub verifying_key: [u8; 32],
    pub signature: SignatureBytes,
}

#[allow(unused)]
const TYPED_OBJECT_ID: UUID = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq)]
pub struct TypedObject {
    pub uuid: UUID,
    pub data: Vec<u8>,
}

#[allow(unused)]
const SIGNED_OBJECT_ID: UUID = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SignedObject {
    pub object: TypedObject,
    pub signature: Signature,
}

#[allow(unused)]
const GROUP_OBJECT_ID: UUID = [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct GroupObject {
    pub group: GroupId,
    pub object: SignedObject,
}

#[allow(unused)]
const PLAIN_FILE_OBJECT_ID: UUID = [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct PlainFileObject {
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

#[allow(unused)]
const EMPTY_OBJECT_ID: UUID = [6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct EmptyObject {}
impl TypedObject {
    pub fn empty() -> Self {
        TypedObject {
            uuid: EMPTY_OBJECT_ID,
            data: bincode::serialize(&EmptyObject {}).unwrap(),
        }
    }
}

const QUERY_OBJECT_ID: UUID = [16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Query {
    pub(crate) query_object: TypedObject,
}
impl From<Query> for TypedObject {
    fn from(obj: Query) -> Self {
        TypedObject {
            uuid: QUERY_OBJECT_ID,
            data: bincode::serialize(&obj).unwrap(),
        }
    }
}

#[allow(unused)]
const SIMPLE_ID_QUERY_ID: UUID = [8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SimpleIDQuery {
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
