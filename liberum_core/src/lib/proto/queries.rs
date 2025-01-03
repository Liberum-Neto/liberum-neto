use super::{Hash, SerializablePublicKey, TypedObject, UUIDTyped};
use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimpleIDQuery {
    pub id: Hash,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeleteObjectQuery {
    pub id: Hash,
    pub verification_key_ed25519: SerializablePublicKey,
}
impl DeleteObjectQuery {
    pub const UUID: Uuid = uuid!("0193b1a3-0b17-73a4-941c-5c79ac9a3780");
}
impl UUIDTyped for DeleteObjectQuery {
    fn get_type_uuid(&self) -> Uuid {
        DeleteObjectQuery::UUID
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PinQuery {
    pub pinned_id: Option<Hash>,
    pub relation: Option<Hash>,
    pub object: TypedObject,
}
impl PinQuery {
    pub const UUID: Uuid = uuid!("01942cf4-6b0b-7c82-aff3-5b5fade8d421");
}

impl UUIDTyped for PinQuery {
    fn get_type_uuid(&self) -> Uuid {
        PinQuery::UUID
    }
}
