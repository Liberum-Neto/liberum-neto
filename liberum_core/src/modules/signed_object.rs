use crate::vaultv3::{self, StoreObject, Vaultv3};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use kameo::actor::ActorRef;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{signed::SignedObject, Hash, TypedObject},
};
use uuid::Uuid;

pub struct SignedObjectModule {
    pub vault: ActorRef<Vaultv3>,
}

#[async_trait]
impl Module for SignedObjectModule {
    async fn publish(
        &self,
        object: TypedObject,
    ) -> Result<(Option<TypedObject>, Option<Vec<Hash>>)> {
        if let ObjectEnum::Signed(obj) = parse_typed(object).await? {
            let typed_object: TypedObject = obj.clone().into();
            let vec_hash = vec![Hash::try_from(&typed_object)?];
            return Ok((Some(obj.object), Some(vec_hash)));
        }
        return Err(anyhow!("Error parsing Signed Object"));
    }

    async fn store(&self, params: ModuleStoreParams) -> Result<ModuleStoreParams> {
        let obj = params.object.unwrap();
        let hash: Hash = Hash::try_from(&obj)?;

        if let ObjectEnum::Signed(obj) = parse_typed(obj).await? {
            let mut hashes = params.signed_objects_hashes;
            let typed_object: TypedObject = obj.clone().into();
            let result = self
                .vault
                .ask(StoreObject {
                    hash: hash,
                    object: obj.clone().into(),
                })
                .await
                .unwrap_or_default();
            hashes.push(Hash::try_from(&typed_object)?);
            if result {
                return Ok(ModuleStoreParams {
                    object: Some(obj.object),
                    signed_objects_hashes: hashes,
                });
            } else {
                // skip object exist, no parsing
                return Ok(ModuleStoreParams {
                    object: None,
                    signed_objects_hashes: hashes,
                });
            }
        } else {
            return Err(anyhow!("Error parsing Signed Object"));
        }
    }

    async fn query(&self, params: ModuleQueryParams) -> Result<ModuleQueryParams> {
        if let ObjectEnum::Signed(obj) = parse_typed(params.object.unwrap()).await? {
            match parse_typed(obj.object).await? {
                ObjectEnum::DeleteObject(del) => {
                    if let Some(typed) = self
                        .vault
                        .ask(vaultv3::RetrieveObject {
                            hash: del.id.clone(),
                        })
                        .await?
                    {
                        if let ObjectEnum::Signed(signed) = parse_typed(typed).await? {
                            let valid_delete =
                                signed.verify_ed25519(del.verification_key_ed25519.try_into()?)?;

                            if valid_delete {
                                // TODO: do smt with this
                                let _is_success = __self
                                    .vault
                                    .ask(vaultv3::DeleteObject { hash: del.id })
                                    .await?;
                            }
                        }
                    }
                }
                _ => {}
            }
            return Ok(ModuleQueryParams {
                matched_object_id: params.matched_object_id,
                object: None,
            });
        } else {
            return Err(anyhow!("Error parsing Signed Object"));
        }
    }

    fn register_module(&self) -> Vec<Uuid> {
        return vec![SignedObject::UUID];
    }
}
