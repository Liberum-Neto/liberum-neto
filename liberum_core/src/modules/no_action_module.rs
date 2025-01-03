use anyhow::Result;
use async_trait::async_trait;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    proto::{Hash, TypedObject},
};
use uuid::Uuid;

pub struct NoActionModule {}

#[async_trait]
impl Module for NoActionModule {
    async fn publish(
        &self,
        _object: TypedObject,
    ) -> Result<(Option<TypedObject>, Option<Vec<Hash>>)> {
        return Ok((None, None));
    }

    async fn store(&self, params: ModuleStoreParams) -> Result<ModuleStoreParams> {
        Ok(ModuleStoreParams {
            object: None,
            signed_objects_hashes: params.signed_objects_hashes,
        })
    }

    async fn query(&self, params: ModuleQueryParams) -> Result<ModuleQueryParams> {
        Ok(ModuleQueryParams {
            matched_object_id: params.matched_object_id,
            object: None,
            return_objects: params.return_objects,
        })
    }

    fn register_module(&self) -> Vec<Uuid> {
        return vec![];
    }
}
