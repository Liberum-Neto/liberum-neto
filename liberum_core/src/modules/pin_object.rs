use crate::vaultv3::{self, Vaultv3};
use async_trait::async_trait;
use kameo::actor::ActorRef;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{pins::PinObject, Hash, TypedObject},
};
use uuid::Uuid;

pub struct PinObjectModule {
    pub vault: ActorRef<Vaultv3>,
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
            let result = self
                .vault
                .ask(vaultv3::StorePin {
                    from_object_hash: obj.pinned_id,
                    main_object_hash: params.signed_objects_hashes[0].clone(),
                    relation_object_hash: obj.relation,
                })
                .await;
            result.unwrap();

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
            let matching_pins = self
                .vault
                .ask(vaultv3::MatchingPins {
                    main_object_hashes: params.matched_object_id,
                    from_object_hash: Some(obj.pinned_id),
                    relation_object_hash: obj.relation,
                })
                .await
                .unwrap();

            ModuleQueryParams {
                matched_object_id: Some(matching_pins),
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
