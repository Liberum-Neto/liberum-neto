use crate::node;
use crate::proto::*;
use crate::swarm_runner::behaviour::BehaviourContext;
use futures::future::{BoxFuture, FutureExt};
use kameo::actor::ActorRef;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use tokio::sync::Mutex;
use tracing::debug;

fn boxed_parse_typed(
    object: TypedObject,
    node: ActorRef<node::Node>,
    swarm_context: Arc<Mutex<BehaviourContext>>,
) -> BoxFuture<'static, TypedObject> {
    parse_typed(object, node, swarm_context.clone()).boxed()
}

pub(crate) async fn parse_typed(
    object: TypedObject,
    node: ActorRef<node::Node>,
    swarm_context: Arc<Mutex<BehaviourContext>>,
) -> TypedObject {
    match object.uuid {
        GROUP_OBJECT_ID => {
            debug!("Parser: Group object: {:?}", object);
            let group_object: GroupObject = bincode::deserialize(&object.data).unwrap();
            let group = group_object.group;
            let signed_object = group_object.object;
            parse_signed(signed_object);
            TypedObject::empty()
        }
        SIGNED_OBJECT_ID => {
            debug!("Parser: Signed object: {:?}", object);
            let signed_object: SignedObject = bincode::deserialize(&object.data).unwrap();
            let object = signed_object.object;
            let signature = signed_object.signature;
            boxed_parse_typed(object, node, swarm_context).await;
            TypedObject::empty()
        }
        TYPED_OBJECT_ID => {
            debug!("Parser: Typed object: {:?}", object);
            let typed_object: TypedObject = bincode::deserialize(&object.data).unwrap();
            boxed_parse_typed(typed_object, node, swarm_context).await;
            TypedObject::empty()
        }
        PLAIN_FILE_OBJECT_ID => {
            debug!("Parser: Plain File Object: {:?}", object);
            let plain_file_object: PlainFileObject = bincode::deserialize(&object.data).unwrap();
            let resp = node
                .ask(node::ProvideObject {
                    object: plain_file_object.into(),
                })
                .await;

            if let Ok(resp) = resp {
                debug!("Provided object: {:?}", resp);
            } else {
                debug!("Failed to provide object: {:?}", resp);
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
            let res = node
                .ask(node::GetObjectFromVault { key: query.id })
                .await
                .unwrap()
                .unwrap();
            res
        }
        QUERY_OBJECT_ID => {
            debug!("Parser: Got Query object: {:?}", object);
            let query: Query = bincode::deserialize(&object.data).unwrap();
            boxed_parse_typed(query.query_object, node, swarm_context).await
        }
        _ => {
            debug!("Parser: Unknown object: {:?}", object);
            TypedObject::empty()
        }
    }
}

fn parse_signed(signed: SignedObject) {}
