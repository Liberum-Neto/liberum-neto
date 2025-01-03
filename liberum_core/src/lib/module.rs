use async_trait::async_trait;
use uuid::Uuid;

use crate::{Hash, TypedObject};
use anyhow::Result;
pub struct ModuleStoreParams {
    pub signed_objects_hashes: Vec<Hash>,
    pub object: Option<TypedObject>,
}

pub struct ModuleQueryParams {
    pub object: Option<TypedObject>,
    // if none then from all match with filter
    // if some then return subset of matched before
    pub matched_object_id: Option<Vec<Hash>>,
    pub return_objects: Vec<TypedObject>,
}

#[async_trait]
pub trait Module {
    // called when trying to publish object, to list
    // return unwrapped object as input for next handler
    // and hashes of places to put object into
    async fn publish(
        &self,
        object: TypedObject,
    ) -> Result<(Option<TypedObject>, Option<Vec<Hash>>)>;

    // store is called when object is to be stored
    // object can pass throught this and still be deleted at the end
    // this function must unwrap object to next typed object
    async fn store(&self, params: ModuleStoreParams) -> Result<ModuleStoreParams>;

    async fn query(&self, params: ModuleQueryParams) -> Result<ModuleQueryParams>;

    // functions needed to auto load module

    fn register_module(&self) -> Vec<Uuid>;
}
