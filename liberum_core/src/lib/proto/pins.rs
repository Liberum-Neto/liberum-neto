use super::{Hash, TypedObject, UUIDTyped};
use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};

#[derive(Serialize, Deserialize, Debug)]
pub struct PinObject {
    pub pinned_id: Hash,
    pub relation: Option<Hash>,
    pub object: TypedObject,
}
impl PinObject {
    pub const UUID: Uuid = uuid!("019418f0-c213-797f-be97-732154fcff12");
}

impl UUIDTyped for PinObject {
    fn get_type_uuid(&self) -> Uuid {
        PinObject::UUID
    }
}
