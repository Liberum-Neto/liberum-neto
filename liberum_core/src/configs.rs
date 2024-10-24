use libp2p::identity::Keypair;
use homedir;
use serde::{Deserialize, Serialize};
use tracing::{debug,info,error};
use std::{fs, io::Write, os::unix::fs::{DirBuilderExt, PermissionsExt}, path::{self, PathBuf}};

const LN_CONFIG_DIRECTORY: &str = ".liberum-neto";

#[derive(Debug)]
pub struct Config {
    pub path: path::PathBuf,
    pub identity: Keypair,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConfigSerializable {
    pub path: path::PathBuf,
    pub identity: Vec<u8>,
}

fn path_or_default(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(|| {
        homedir::my_home().unwrap()
        .expect("Should be able to find the home path")
        .join(LN_CONFIG_DIRECTORY)
        .join("default-node.lnc")
    })

}

impl Config {
    pub fn new(path: Option<PathBuf>) -> Self {
        let path = path_or_default(path);
        debug!("Creating config struct at {path:?}");
        Self{path: path, identity: Keypair::generate_ed25519()}
    }

    pub fn get_identity(&self) -> Keypair {
        libp2p::identity::Keypair::generate_ed25519()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>>{
        debug!("Saving config at {:?}",self.path);
        if let Some(parent) = self.path.parent() {
            debug!("parent: {parent:?}");
            std::fs::DirBuilder::new().recursive(true).mode(0o711).create(&parent).unwrap();
        } else {
            panic!("No parent");
        }
        
        let mut file = fs::File::create(&self.path).unwrap();
        file.set_permissions(fs::Permissions::from_mode(0o600)).unwrap();
        let serializable = self.as_serializable();
        file.write_all(serde_json::to_string(&serializable)?.as_bytes())?;
        debug!("Success in writing the {serializable:?} config to {:?}", self.path);
        Ok(())
    }


    pub fn load(path: Option<PathBuf>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path_or_default(path);
        let file = fs::File::open(&path)?;
        let serializable = serde_json::from_reader::<fs::File, ConfigSerializable>(file)?;
        let config = Self::from_serializable(&serializable);
        debug!("Success in reading {serializable:?} from {path:?}");
        config
    }

    pub fn as_serializable(&self) -> ConfigSerializable {
        ConfigSerializable {
            path: self.path.clone(),
            identity: self.identity.to_protobuf_encoding().unwrap(),
        }
    }
    pub fn from_serializable(config: &ConfigSerializable) -> Result<Self, Box<dyn std::error::Error>> {
        let id = match Keypair::from_protobuf_encoding(&config.identity) {
            Ok(pair) => pair,
            Err(e) => {
                error!("Error when deserializing config: {e:?}");
                return Err(Box::new(e));
            }
        };
        Ok(Config {
            path: config.path.clone(),
            identity: id,
        })
    }


}