use crate::proto::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use strum_macros::Display;
use tracing::debug;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Display)]
pub enum ObjectEnum {
    Group(GroupObject),
    Signed(SignedObject),
    PlainFile(PlainFileObject),
    Typed(TypedObject),
    Empty(EmptyObject),
    SimpleIDQuery(SimpleIDQuery),
    Query(QueryObject),
    Result(ResultObject),
}
impl UUIDTyped for ObjectEnum {
    // TODO couldn't we do this better? Is it possible to force a member of an enum to implement a trait??
    fn get_type_uuid(&self) -> Uuid {
        match self {
            ObjectEnum::Group(group_object) => group_object.get_type_uuid(),
            ObjectEnum::Signed(signed_object) => signed_object.get_type_uuid(),
            ObjectEnum::PlainFile(plain_file_object) => plain_file_object.get_type_uuid(),
            ObjectEnum::Typed(typed_object) => typed_object.get_uuid(),
            ObjectEnum::Empty(empty_object) => empty_object.get_type_uuid(),
            ObjectEnum::SimpleIDQuery(simple_idquery) => simple_idquery.get_type_uuid(),
            ObjectEnum::Query(query_object) => query_object.get_type_uuid(),
            ObjectEnum::Result(result_object) => result_object.get_type_uuid(),
        }
    }
}

pub async fn parse_typed(object: TypedObject) -> Result<ObjectEnum> {
    match object.uuid {
        GroupObject::UUID => {
            debug!("Parser: Group object: {:?}", object);
            todo!()
        }
        SignedObject::UUID => {
            debug!("Parser: Signed object: {:?}", object);
            todo!()
        }
        PlainFileObject::UUID => {
            debug!("Parser: Plain File Object: {:?}", object);
            let plain_file_object = PlainFileObject::try_from(&object)?;
            Ok(ObjectEnum::PlainFile(plain_file_object))
        }
        EmptyObject::UUID => {
            debug!("Parser: Empty Object: {:?}", object);
            // Do nothing
            Ok(ObjectEnum::Empty(EmptyObject {}))
        }
        SimpleIDQuery::UUID => {
            debug!("Parser: Simple ID Query: {:?}", object);
            let query = SimpleIDQuery::try_from(&object).unwrap();
            Ok(ObjectEnum::SimpleIDQuery(query))
        }
        QueryObject::UUID => {
            debug!("Parser: Got Query object: {:?}", object);
            let query = QueryObject::try_from(&object).unwrap();
            Ok(ObjectEnum::Query(QueryObject {
                query_object: query.query_object,
            }))
        }
        ResultObject::UUID => {
            debug!("Parser: Got Result object: {:?}", object);
            let obj = ResultObject::try_from(&object).unwrap();
            Ok(ObjectEnum::Result(obj))
        }
        _ => {
            debug!("Parser: Unknown object: {:?}", object);
            Ok(ObjectEnum::Empty(EmptyObject {}))
        }
    }
}
