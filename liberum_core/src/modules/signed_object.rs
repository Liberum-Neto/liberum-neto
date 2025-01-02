use async_trait::async_trait;
use kameo::actor::ActorRef;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{signed::SignedObject, Hash, TypedObject},
};
use uuid::Uuid;

use crate::vaultv3::{self, StoreObject, Vaultv3};

pub struct SignedObjectModule {
    pub vault: ActorRef<Vaultv3>,
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
        let obj = params.object.unwrap();
        let hash: Hash = Hash::try_from(&obj).unwrap();

        if let ObjectEnum::Signed(obj) = parse_typed(obj).await.unwrap() {
            let mut vec = params.signed_objects_hashes;
            let typed_object: TypedObject = obj.clone().into();
            let result = self
                .vault
                .ask(StoreObject {
                    hash: hash,
                    object: obj.clone(),
                })
                .await
                .unwrap_or_default();
            vec.push(Hash::try_from(&typed_object).unwrap());
            if result {
                ModuleStoreParams {
                    object: Some(obj.object),
                    signed_objects_hashes: vec,
                }
            } else {
                // skip object exist, no parsing
                ModuleStoreParams {
                    object: None,
                    signed_objects_hashes: vec,
                }
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
            match parse_typed(obj.object).await.unwrap() {
                ObjectEnum::DeleteObject(del) => {
                    if let Some(signed) = self
                        .vault
                        .ask(vaultv3::RetriveObject {
                            hash: del.id.clone(),
                        })
                        .await
                        .unwrap()
                    {
                        let valid_delete = signed
                            .verify_ed25519(del.verification_key_ed25519.try_into().unwrap())
                            .unwrap();

                        if valid_delete {
                            // TODO: do smt with this
                            let _is_success = __self
                                .vault
                                .ask(vaultv3::DeleteObject { hash: del.id })
                                .await
                                .unwrap();
                        }
                    }
                    ModuleQueryParams {
                        matched_object_id: params.matched_object_id,
                        object: None,
                    }
                }
                _ => ModuleQueryParams {
                    matched_object_id: params.matched_object_id,
                    object: None,
                },
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
