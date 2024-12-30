use crate::vault::Vault;
use async_trait::async_trait;
use kameo::actor::ActorRef;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{pins::PinObject, Hash, TypedObject},
};
use uuid::Uuid;

pub struct PinObjectModule {
    pub vault: ActorRef<Vault>,
}

#[async_trait]
impl Module for PinObjectModule {
    async fn publish(&self, object: TypedObject) -> (Option<TypedObject>, Option<Vec<Hash>>) {
        if let ObjectEnum::Pin(obj) = parse_typed(object).await.unwrap() {
            return (Some(obj.object), Some(vec![obj.pinned_id]));
        }
        return (None, None);
    }

    async fn store(&self, params: ModuleStoreParams) -> ModuleStoreParams {
        if let ObjectEnum::Pin(obj) = parse_typed(params.object.unwrap()).await.unwrap() {
            // TODO save to vault
            return ModuleStoreParams {
                signed_objects_hashes: params.signed_objects_hashes,
                object: Some(obj.object),
            };
        }
        return ModuleStoreParams {
            signed_objects_hashes: vec![],
            object: None,
        };
    }

    async fn query(&self, params: ModuleQueryParams) -> ModuleQueryParams {
        if let ObjectEnum::Pin(obj) = parse_typed(params.object.unwrap()).await.unwrap() {
            ModuleQueryParams {
                matched_object_id: params.matched_object_id,
                object: Some(obj.object),
            }
        } else {
            ModuleQueryParams {
                matched_object_id: params.matched_object_id,
                object: None,
            }
        }
    }

    fn register_module(&self) -> Vec<Uuid> {
        return vec![PinObject::UUID];
    }
}
