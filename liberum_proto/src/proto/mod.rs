mod modules;
use serde::{Serialize, Deserialize};
pub(crate) type Hash = [u8; 32];
pub(crate) type UserId = Hash;
pub(crate) type GroupId = Hash;
pub(crate) type ObjectId = Hash;
pub(crate) type Content = Vec<u8>;
pub(crate) type UUID = [u8; 16];

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
#[derive(Serialize, Deserialize, Debug)]
type SignatureBytes = [u8; 64];


#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SignatureEd25519 {
    pub verifying_key: [u8; 32],
    pub signature: SignatureBytes,
}

const TYPED_OBJECT_ID: UUID = [3,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct TypedObject {
    pub uuid: UUID,
    pub data: Vec<u8>,
}
const SIGNED_OBJECT_ID: UUID = [2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SignedObject {
    pub object: TypedObject,
    pub signature: Signature,
}

const GROUP_OBJECT_ID: UUID = [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct GroupObject {
    group: GroupId,
    object: SignedObject,
}

