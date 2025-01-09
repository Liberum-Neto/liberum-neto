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
impl PinObject {
    pub fn extend(self, pinned_id: Hash, relation: Option<Hash>) -> Self {
        PinObject {
            pinned_id,
            relation,
            object: self.into(),
        }
    }

    pub fn add_pins(mut object: TypedObject, pins: Vec<(Hash, Option<Hash>)>) -> TypedObject {
        for pin in pins {
            object = PinObject {
                pinned_id: pin.0,
                relation: pin.1,
                object: object,
            }
            .into();
        }
        object
    }
}
