use crate::vaultv3::{self, Vaultv3};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use kameo::actor::ActorRef;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{pins::PinObject, queries::PinQuery, Hash, TypedObject},
};
use uuid::Uuid;

pub struct PinObjectModule {
    pub vault: ActorRef<Vaultv3>,
}

#[async_trait]
impl Module for PinObjectModule {
    async fn publish(
        &self,
        object: TypedObject,
    ) -> Result<(Option<TypedObject>, Option<Vec<Hash>>)> {
        match parse_typed(object).await? {
            ObjectEnum::Pin(obj) => {
                let locations = if let Some(relation) = obj.relation {
                    vec![obj.pinned_id, relation]
                } else {
                    vec![obj.pinned_id]
                };

                return Ok((Some(obj.object), Some(locations)));
            }
            ObjectEnum::PinQuery(query) => {
                let mut locations = Vec::with_capacity(2);

                if let Some(relation) = query.relation {
                    locations.push(relation);
                }
                if let Some(pin) = query.pinned_id {
                    locations.push(pin);
                }
                return Ok((Some(query.object), Some(locations)));
            }
            _ => (),
        }
        return Err(anyhow!("Error parsing PinObject"));
    }

    async fn store(&self, params: ModuleStoreParams) -> Result<ModuleStoreParams> {
        if let ObjectEnum::Pin(obj) = parse_typed(params.object.unwrap()).await? {
            let result = self
                .vault
                .ask(vaultv3::StorePin {
                    from_object_hash: obj.pinned_id,
                    main_object_hash: params.signed_objects_hashes[0].clone(),
                    relation_object_hash: obj.relation,
                })
                .await;
            result.unwrap();

            return Ok(ModuleStoreParams {
                signed_objects_hashes: params.signed_objects_hashes,
                object: Some(obj.object),
            });
        }
        return Err(anyhow!("Error parsing PinObject"));
    }

    async fn query(&self, params: ModuleQueryParams) -> Result<ModuleQueryParams> {
        if let ObjectEnum::PinQuery(obj) = parse_typed(params.object.unwrap()).await? {
            let matching_pins = self
                .vault
                .ask(vaultv3::MatchingPins {
                    main_object_hashes: params.matched_object_id,
                    from_object_hash: obj.pinned_id,
                    relation_object_hash: obj.relation,
                })
                .await?;

            return Ok(ModuleQueryParams {
                matched_object_id: Some(matching_pins),
                object: Some(obj.object),
                return_objects: params.return_objects,
            });
        }
        return Err(anyhow!("Error parsing PinObject"));
    }

    fn register_module(&self) -> Vec<Uuid> {
        return vec![PinObject::UUID, PinQuery::UUID];
    }
}
