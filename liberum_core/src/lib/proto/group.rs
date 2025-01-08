use super::signed::SignedObject;
use super::{Hash, Signature, UUIDTyped};
use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupAccessToken {
    pub binding: GroupAccessBinding,
    pub group_owner_signature: Signature,
}

pub type UnixTimestamp = u64;

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupAccessBinding {
    pub group_id: Hash,
    pub recipient_id: Hash,
    pub revocation_date: UnixTimestamp,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserGroup {
    pub definition: GroupDefinition,
    pub signature: Signature,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupDefinition {
    pub group_id: Hash,
    pub owner_id: Hash,
    pub parent_id: Hash,
    pub parent_membership_proof: GroupAccessToken,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupObject {
    pub group_id: Hash,
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
