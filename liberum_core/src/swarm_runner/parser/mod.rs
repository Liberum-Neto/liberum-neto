use crate::proto::*;
use crate::swarm_runner::SwarmContext;
use futures::future::BoxFuture;
use futures::FutureExt;
use tokio::sync::oneshot;
use tracing::debug;

impl SwarmContext {
    fn boxed_parse_typed<'a>(&'a mut self, object: TypedObject) -> BoxFuture<'a, TypedObject> {
        self.parse_typed(object).boxed()
    }
    pub(crate) async fn parse_typed(&mut self, object: TypedObject) -> TypedObject {
        match object.uuid {
            GROUP_OBJECT_ID => {
                debug!("Parser: Group object: {:?}", object);
                let group_object: GroupObject = bincode::deserialize(&object.data).unwrap();
                let group = group_object.group;
                let signed_object = group_object.object;
                self.parse_signed(signed_object);
                TypedObject::empty()
            }
            SIGNED_OBJECT_ID => {
                debug!("Parser: Signed object: {:?}", object);
                let signed_object: SignedObject = bincode::deserialize(&object.data).unwrap();
                let object = signed_object.object;
                let signature = signed_object.signature;
                self.boxed_parse_typed(object).await;
                TypedObject::empty()
            }
            TYPED_OBJECT_ID => {
                debug!("Parser: Typed object: {:?}", object);
                let typed_object: TypedObject = bincode::deserialize(&object.data).unwrap();
                self.boxed_parse_typed(typed_object).await;
                TypedObject::empty()
            }
            PLAIN_FILE_OBJECT_ID => {
                debug!("Parser: Plain File Object: {:?}", object);
                let plain_file_object: PlainFileObject =
                    bincode::deserialize(&object.data).unwrap();
                let (s, r) = oneshot::channel();
                self.provide_object(object.clone(), s).await;

                if let Ok(_) = r.await {
                    debug!("Provided object: {:?}", object);
                } else {
                    debug!("Failed to provide object: {:?}", object);
                }
                TypedObject::empty()
            }
            EMPTY_OBJECT_ID => {
                debug!("Parser: Empty Object: {:?}", object);
                // Do nothing
                TypedObject::empty()
            }
            SIMPLE_ID_QUERY_ID => {
                debug!("Parser: Simple ID Query: {:?}", object);
                let query: SimpleIDQuery = bincode::deserialize(&object.data).unwrap();
                if let Some(obj) = self.get_object_from_vault(query.id) {
                    obj
                } else {
                    TypedObject::empty()
                }
            }
            QUERY_OBJECT_ID => {
                debug!("Parser: Got Query object: {:?}", object);
                let query: Query = bincode::deserialize(&object.data).unwrap();
                self.boxed_parse_typed(query.query_object).await
            }
            _ => {
                debug!("Parser: Unknown object: {:?}", object);
                TypedObject::empty()
            }
        }
    }

    fn parse_signed(&mut self, signed: SignedObject) {}
}
