use async_trait::async_trait;
use kameo::actor::ActorRef;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{signed::SignedObject, Hash, TypedObject},
};
use uuid::Uuid;

use crate::vault::Vault;

pub struct SignedObjectModule {
    pub vault: ActorRef<Vault>,
}

#[async_trait]
impl Module for SignedObjectModule {
    async fn publish(&self, object: TypedObject) -> (Option<TypedObject>, Option<Vec<Hash>>) {
        if let ObjectEnum::Signed(obj) = parse_typed(object).await.unwrap() {
            // self.vault.ask() // nie ten vault więc tylko tutaj to zostawie

            let typed_object: TypedObject = obj.clone().into();
            let vec_hash = vec![Hash::try_from(&typed_object).unwrap()];
            return (Some(obj.object), Some(vec_hash));
        }
        return (None, None);
    }

    async fn store(&self, params: ModuleStoreParams) -> ModuleStoreParams {
        if let ObjectEnum::Signed(obj) = parse_typed(params.object.unwrap()).await.unwrap() {
            let mut vec = params.signed_objects_hashes;
            let typed_object: TypedObject = obj.clone().into();
            vec.push(Hash::try_from(&typed_object).unwrap());

            ModuleStoreParams {
                object: Some(obj.object),
                signed_objects_hashes: vec,
            }
        } else {
            ModuleStoreParams {
                object: None,
                signed_objects_hashes: params.signed_objects_hashes,
            }
        }
    }

    async fn query(&self, params: ModuleQueryParams) -> ModuleQueryParams {
        if let ObjectEnum::Signed(obj) = parse_typed(params.object.unwrap()).await.unwrap() {
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
        return vec![SignedObject::UUID];
    }
}
