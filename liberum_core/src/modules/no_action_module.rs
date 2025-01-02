use async_trait::async_trait;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    proto::{Hash, TypedObject},
};
use uuid::Uuid;

pub struct NoActionModule {}

#[async_trait]
impl Module for NoActionModule {
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
        ModuleQueryParams {
            matched_object_id: params.matched_object_id,
            object: None,
        }
    }

    fn register_module(&self) -> Vec<Uuid> {
        return vec![];
    }
}
