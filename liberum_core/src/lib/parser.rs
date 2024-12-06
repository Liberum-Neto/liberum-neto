use crate::proto::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use strum_macros::Display;
use tracing::debug;

#[derive(Serialize, Deserialize, Debug, Display)]
pub enum ObjectEnum {
    Group(GroupObject),
    Signed(SignedObject),
    PlainFile(PlainFileObject),
    Typed(TypedObjectOld),
    Empty(EmptyObject),
    SimpleIDQuery(SimpleIDQuery),
    Query(QueryObject),
    Result(ResultObject),
}

pub async fn parse_typed(object: TypedObjectOld) -> Result<ObjectEnum> {
    match object.uuid {
        GROUP_OBJECT_ID => {
            debug!("Parser: Group object: {:?}", object);
            todo!()
        }
        SIGNED_OBJECT_ID => {
            debug!("Parser: Signed object: {:?}", object);
            todo!()
        }
        PLAIN_FILE_OBJECT_ID => {
            debug!("Parser: Plain File Object: {:?}", object);
            let plain_file_object: PlainFileObject = bincode::deserialize(&object.data).unwrap();
            Ok(ObjectEnum::PlainFile(plain_file_object))
        }
        EMPTY_OBJECT_ID => {
            debug!("Parser: Empty Object: {:?}", object);
            // Do nothing
            Ok(ObjectEnum::Empty(EmptyObject {}))
        }
        SIMPLE_ID_QUERY_ID => {
            debug!("Parser: Simple ID Query: {:?}", object);
            let query: SimpleIDQuery = bincode::deserialize(&object.data).unwrap();
            Ok(ObjectEnum::SimpleIDQuery(query))
        }
        QUERY_OBJECT_ID => {
            debug!("Parser: Got Query object: {:?}", object);
            let query: QueryObject = bincode::deserialize(&object.data).unwrap();
            Ok(ObjectEnum::Query(QueryObject {
                query_object: query.query_object,
            }))
        }
        RESULT_OBJECT_ID => {
            debug!("Parser: Got Result object: {:?}", object);
            let obj: ResultObject = bincode::deserialize(&object.data).unwrap();
            Ok(ObjectEnum::Result(obj))
        }
        _ => {
            debug!("Parser: Unknown object: {:?}", object);
            Ok(ObjectEnum::Empty(EmptyObject {}))
        }
    }
}
