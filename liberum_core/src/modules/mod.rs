pub mod no_action_module;
pub mod pin_object;
pub mod plain_file_object;
pub mod signed_object;
pub mod simple_id_query_object;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::{anyhow, Result};
use kameo::actor::ActorRef;
use liberum_core::{
    module::{Module, ModuleQueryParams, ModuleStoreParams},
    proto::{self, TypedObject},
};
use plain_file_object::PlainFileObjectModule;
use signed_object::SignedObjectModule;
use simple_id_query_object::SimpleIDQueryModule;
use uuid::Uuid;

use crate::vaultv3::Vaultv3;

pub struct Modules {
    installed_modules: HashMap<Uuid, Arc<Box<dyn Module + Send + Sync>>>,
}

impl Modules {
    pub fn install_module(&mut self, module: Arc<Box<dyn Module + Send + Sync>>) {
        for uuid in module.register_module() {
            self.installed_modules.insert(uuid, module.clone());
        }
    }

    pub fn new() -> Modules {
        Modules {
            installed_modules: HashMap::new(),
        }
    }

    // in external function you must publish input file in locations specified (get provider results)
    pub async fn publish(&self, object: TypedObject) -> Result<Vec<proto::Hash>> {
        let mut obj = object;
        let mut publish_places: HashSet<proto::Hash> = HashSet::new();

        while let Some(module) = self.installed_modules.get(&obj.uuid) {
            let (object, places) = module.publish(obj).await?;

            if let Some(places) = places {
                for ele in places {
                    publish_places.insert(ele);
                }
            };

            if let Some(object) = object {
                obj = object;
            } else {
                break;
            }
        }

        let mut vec = Vec::new();
        for ele in publish_places {
            vec.push(ele);
        }
        return Ok(vec);
    }

    // before calling add object to vault
    // if false then remove object from vault
    pub async fn store(&self, object: TypedObject) -> Result<bool> {
        let mut params = ModuleStoreParams {
            object: Some(object),
            signed_objects_hashes: Vec::new(),
        };

        while let Some(obj) = &params.object {
            if let Some(module) = self.installed_modules.get(&obj.uuid) {
                params = module.store(params).await?;

                if params.signed_objects_hashes.len() == 0 {
                    return Err(anyhow!("Object not Signed")); // first object must be signed
                }
            }
        }
        Ok(true)
    }

    // map these values into objects from vault
    pub async fn query(&self, object: TypedObject) -> Result<Vec<proto::Hash>> {
        let mut params = ModuleQueryParams {
            matched_object_id: None,
            object: Some(object),
        };
        while let Some(obj) = &params.object {
            if let Some(module) = self.installed_modules.get(&obj.uuid) {
                params = module.query(params).await?;

                if let Some(matches) = &params.matched_object_id {
                    if matches.len() == 0 {
                        return Ok(matches.to_vec());
                    }
                }
            }
        }
        if let Some(matches) = params.matched_object_id {
            return Ok(matches);
        }
        return Ok(vec![]);
    }
}

impl Modules {
    pub fn install_default(&mut self, vault: ActorRef<Vaultv3>) {
        // this can only be done without need for more actors (this will need to be in other file)
        self.install_module(Arc::new(Box::new(SignedObjectModule {
            vault: vault.clone(),
        })));
        self.install_module(Arc::new(Box::new(SimpleIDQueryModule {})));
        self.install_module(Arc::new(Box::new(PlainFileObjectModule {})));
    }
}
