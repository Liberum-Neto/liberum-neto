use std::{fmt::Display, path::Path};

use anyhow::bail;
use anyhow::{anyhow, Error, Result};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uuid::{uuid, Uuid};

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Clone, Eq)]
pub struct Hash {
    pub bytes: [u8; 32],
}
pub type UserId = Hash;
pub type GroupId = Hash;
pub type ObjectId = Hash;
pub type Content = Vec<u8>;

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
impl From<&[u8; 32]> for Hash {
    fn from(value: &[u8; 32]) -> Self {
        Hash {
            bytes: value.clone(),
        }
    }
}
impl TryFrom<&TypedObject> for Hash {
    type Error = Error;

    fn try_from(value: &TypedObject) -> Result<Self> {
        Ok(blake3::hash(bincode::serialize(value)?.as_slice())
            .as_bytes()
            .into())
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedObject {
    pub object: TypedObject,
    pub signature: Signature,
}
impl SignedObject {
    pub const UUID: Uuid = uuid!("0193a7bf-fb8f-7fdc-8be6-02d3a3cc7eb1");
}
impl UUIDTyped for SignedObject {
    fn get_type_uuid(&self) -> Uuid {
        SignedObject::UUID
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupObject {
    pub group: GroupId,
    pub object: SignedObject,
}
impl GroupObject {
    pub const UUID: Uuid = uuid!("0193a7c0-1cb7-72e8-97cd-e84c15925233");
}
impl UUIDTyped for GroupObject {
    fn get_type_uuid(&self) -> Uuid {
        GroupObject::UUID
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlainFileObject {
    pub name: String,
    pub content: Content,
}
impl PlainFileObject {
    pub const UUID: Uuid = uuid!("0193a7c0-3ad3-707c-897b-f23b30400c69");
}
impl UUIDTyped for PlainFileObject {
    fn get_type_uuid(&self) -> Uuid {
        PlainFileObject::UUID
    }
}
impl TryFrom<&TypedObject> for PlainFileObject {
    type Error = Error;
    fn try_from(value: &TypedObject) -> Result<Self> {
        bincode::deserialize::<PlainFileObject>(&(value.data)).map_err(|e| anyhow!(e))
    }
}
impl PlainFileObject {
    pub async fn try_from_path(path: &Path) -> Result<Self> {
        let name = {
            let name = path.file_name();
            if let None = name {
                bail!("Invalid filename! {}", path.to_string_lossy())
            }
            let name = name.unwrap().to_str();
            if let None = name {
                bail!("Invalid filename! {},", path.to_string_lossy())
            }
            name.unwrap().to_string()
        };

        Ok(PlainFileObject {
            name,
            content: tokio::fs::read(path).await?,
        })
    }
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
pub struct QueryObject {
    pub query_object: TypedObject,
}
impl QueryObject {
    pub const UUID: Uuid = uuid!("0193a7c0-800f-7bba-9524-0244e86fd5dc");
}
impl UUIDTyped for QueryObject {
    fn get_type_uuid(&self) -> Uuid {
        QueryObject::UUID
    }
}
impl TryFrom<&TypedObject> for QueryObject {
    type Error = Error;
    fn try_from(value: &TypedObject) -> Result<Self> {
        bincode::deserialize::<QueryObject>(&(value.data)).map_err(|e| anyhow!(e))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimpleIDQuery {
    pub id: ObjectId,
}
impl SimpleIDQuery {
    pub const UUID: Uuid = uuid!("0193a7c0-9cb7-7184-844e-42b5a1bf999e");
}
impl UUIDTyped for SimpleIDQuery {
    fn get_type_uuid(&self) -> Uuid {
        SimpleIDQuery::UUID
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
impl TryFrom<&TypedObject> for ResultObject {
    type Error = Error;
    fn try_from(value: &TypedObject) -> Result<Self> {
        bincode::deserialize::<ResultObject>(&(value.data)).map_err(|e| anyhow!(e))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PinObject {
    pub from: Hash,
    pub to: TypedObjectRef,
}
impl PinObject {
    pub const UUID: Uuid = uuid!("fdf23e1d-f966-4605-a399-9198bf5870e5");
}
impl UUIDTyped for PinObject {
    fn get_type_uuid(&self) -> Uuid {
        PinObject::UUID
    }
}
impl TryFrom<&TypedObject> for PinObject {
    type Error = Error;

    fn try_from(value: &TypedObject) -> std::result::Result<Self, Self::Error> {
        bincode::deserialize::<PinObject>(&(value.data)).map_err(|e| anyhow!(e))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TypedObjectRef {
    Direct(TypedObject),
    ByHash(Hash),
}
