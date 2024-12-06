use crate::node::Node;
use crate::proto::*;
use kameo::actor::ActorRef;

pub fn parse(object: Vec<u8>, node: ActorRef<Node>) {
    let typed_object: TypedObject = bincode::deserialize(&object).unwrap();
    parse_typed(typed_object);
}

fn parse_typed(object: TypedObject) {
    match object.uuid {
        GROUP_OBJECT_ID => {
            let group_object: GroupObject = bincode::deserialize(&object.data).unwrap();
            let group = group_object.group;
            let signed_object = group_object.object;
            parse_signed(signed_object);
        }
        SIGNED_OBJECT_ID => {
            let signed_object: SignedObject = bincode::deserialize(&object.data).unwrap();
            let object = signed_object.object;
            let signature = signed_object.signature;
            parse_typed(object);
        }
        TYPED_OBJECT_ID => {
            let typed_object: TypedObject = bincode::deserialize(&object.data).unwrap();
            parse_typed(typed_object);
        }
        PLAIN_FILE_OBJECT_ID => {}
        _ => {
            // Do nothing
        }
    }
}

fn parse_signed(signed: SignedObject) {}
