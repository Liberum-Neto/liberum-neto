use anyhow::{anyhow, Result};
use async_trait::async_trait;
use liberum_core::proto::file::PlainFileObject;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{Hash, TypedObject},
};
use uuid::Uuid;

pub struct PlainFileObjectModule {}

#[async_trait]
impl Module for PlainFileObjectModule {
    async fn publish(
        &self,
        object: TypedObject,
    ) -> Result<(Option<TypedObject>, Option<Vec<Hash>>)> {
        if let ObjectEnum::PlainFile(_obj) = parse_typed(object).await? {
            return Ok((None, None));
        }
        return Err(anyhow!("Error parsing Plain File Object"));
    }

    async fn store(&self, params: ModuleStoreParams) -> Result<ModuleStoreParams> {
        if let ObjectEnum::PlainFile(_obj) = parse_typed(params.object.unwrap()).await? {
            // no action
            return Ok(ModuleStoreParams {
                object: None,
                signed_objects_hashes: params.signed_objects_hashes,
            });
        }

        return Err(anyhow!("Error parsing Plain File Object"));
    }

    async fn query(&self, params: ModuleQueryParams) -> Result<ModuleQueryParams> {
        if let ObjectEnum::PlainFile(_obj) = parse_typed(params.object.unwrap()).await? {
            return Ok(ModuleQueryParams {
                matched_object_id: params.matched_object_id,
                object: None, // improper object in query
                return_objects: params.return_objects,
            });
        }
        return Err(anyhow!("Error parsing Plain File Object"));
    }

    fn register_module(&self) -> Vec<Uuid> {
        return vec![PlainFileObject::UUID];
    }
}
