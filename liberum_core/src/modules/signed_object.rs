use crate::vaultv3::{self, Vaultv3};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use kameo::actor::ActorRef;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    parser::{parse_typed, ObjectEnum},
    proto::{signed::SignedObject, Hash, ResultObject, TypedObject},
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
            hashes.push(hash);

            return Ok(ModuleStoreParams {
                object: Some(obj.object),
                signed_objects_hashes: hashes,
            });
        }
        return Err(anyhow!("Error parsing Signed Object"));
    }

    async fn query(&self, params: ModuleQueryParams) -> Result<ModuleQueryParams> {
        if let ObjectEnum::Signed(obj) = parse_typed(params.object.unwrap()).await? {
            match parse_typed(obj.object.clone()).await? {
                ObjectEnum::DeleteObject(del) => {
                    let valid_query = obj.verify_ed25519(&del.verification_key_ed25519)?;
                    if !valid_query {
                        // Invalid query, Signature does not match, don't delete
                        let mut return_objects = params.return_objects;
                        return_objects.push(ResultObject { result: Err(()) }.into());
                        return Ok(ModuleQueryParams {
                            matched_object_id: params.matched_object_id,
                            object: None,
                            return_objects,
                        });
                    }

                    if let Some(typed) = self
                        .vault
                        .ask(vaultv3::RetrieveObject {
                            hash: del.id.clone(),
                        })
                        .await?
                    {
                        if let ObjectEnum::Signed(signed) = parse_typed(typed).await? {
                            let delete_verified =
                                signed.verify_ed25519(&del.verification_key_ed25519)?;

                            if delete_verified {
                                // TODO: do smt with this
                                let _is_success = __self
                                    .vault
                                    .ask(vaultv3::DeleteObject { hash: del.id })
                                    .await?;

                                let mut return_objects = params.return_objects;
                                return_objects.push(ResultObject { result: Ok(()) }.into());
                                return Ok(ModuleQueryParams {
                                    matched_object_id: params.matched_object_id,
                                    object: None,
                                    return_objects,
                                });
                            }
                        }
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "Signed Object in a Query has to have a DeleteObject inside."
                    ));
                }
            }
            let mut return_objects = params.return_objects;
            return_objects.push(ResultObject { result: Err(()) }.into());
            return Ok(ModuleQueryParams {
                matched_object_id: params.matched_object_id,
                object: None,
                return_objects,
            });
        } else {
            return Err(anyhow!("Error parsing Signed Object"));
        }
    }

    fn register_module(&self) -> Vec<Uuid> {
        return vec![SignedObject::UUID];
    }
}
