use crate::vault::Vault;
use async_trait::async_trait;
use kameo::actor::ActorRef;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{queries::SimpleIDQuery, Hash, TypedObject},
};
use uuid::Uuid;

pub struct SimpleIDQueryModule {
    pub vault: ActorRef<Vault>,
}

#[async_trait]
impl Module for SimpleIDQueryModule {
    async fn publish(&self, _object: TypedObject) -> (Option<TypedObject>, Option<Vec<Hash>>) {
        return (None, None);
    }

    async fn store(&self, params: ModuleStoreParams) -> ModuleStoreParams {
        ModuleStoreParams {
            object: None,
            signed_objects_hashes: params.signed_objects_hashes,
        }
    }

    async fn query(&self, params: ModuleQueryParams) -> ModuleQueryParams {
        if let ObjectEnum::SimpleIDQuery(obj) = parse_typed(params.object.unwrap()).await.unwrap() {
            ModuleQueryParams {
                matched_object_id: Some(vec![obj.id]),
                object: None,
            }
        } else {
            ModuleQueryParams {
                matched_object_id: params.matched_object_id,
                object: None,
            }
        }
    }

    fn register_module(&self) -> Vec<Uuid> {
        return vec![SimpleIDQuery::UUID];
    }
}
