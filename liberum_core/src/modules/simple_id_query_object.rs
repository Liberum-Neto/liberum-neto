use anyhow::{anyhow, Result};
use async_trait::async_trait;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{queries::SimpleIDQuery, Hash, TypedObject},
};
use uuid::Uuid;

pub struct SimpleIDQueryModule {}

#[async_trait]
impl Module for SimpleIDQueryModule {
    async fn publish(
        &self,
        object: TypedObject,
    ) -> Result<(Option<TypedObject>, Option<Vec<Hash>>)> {
        if let ObjectEnum::SimpleIDQuery(_obj) = parse_typed(object).await? {
            return Ok((None, None));
        }
        return Err(anyhow!("Error parsing Simple ID Query"));
    }

    async fn store(&self, params: ModuleStoreParams) -> Result<ModuleStoreParams> {
        if let ObjectEnum::SimpleIDQuery(_obj) = parse_typed(params.object.unwrap()).await? {
            return Ok(ModuleStoreParams {
                object: None,
                signed_objects_hashes: params.signed_objects_hashes,
            });
        }
        return Err(anyhow!("Error parsing Simple ID Query"));
    }

    async fn query(&self, params: ModuleQueryParams) -> Result<ModuleQueryParams> {
        if let ObjectEnum::SimpleIDQuery(obj) = parse_typed(params.object.unwrap()).await? {
            return Ok(ModuleQueryParams {
                matched_object_id: Some(vec![obj.id]),
                object: None,
                return_objects: params.return_objects,
            });
        }
        return Err(anyhow!("Error parsing Simple ID Query"));
    }

    fn register_module(&self) -> Vec<Uuid> {
        return vec![SimpleIDQuery::UUID];
    }
}
