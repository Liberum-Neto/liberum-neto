use bincode::config;
use libp2p::identity::Keypair;
use homedir;
use serde::{Deserialize, Serialize};
use tracing::{debug,error};
use void::ResultVoidErrExt;
use std::{fs, io::Write, os::unix::fs::{DirBuilderExt, PermissionsExt}, path::{self, Path, PathBuf}};

/// This module manages saving the configuration of the nodes to the disk.


const LN_CONFIG_DIRECTORY: &str = ".liberum-neto";

/// Manages the configuration of nodes
#[derive(Debug)]
pub struct ConfigManager {
    pub path: path::PathBuf,
}

/// Represents the configuration of a single node
#[derive(Debug)]
pub struct Config {
    pub name: String,
    pub identity: Keypair,
}
/// Serializable version of the Config struct, Keypair does not implement serde
#[derive(Serialize, Deserialize, Debug)]
pub struct ConfigSerializable {
    pub name: String,
    pub identity: Vec<u8>,
}

/// Returns the path or the default path if the path is None
fn path_or_default(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(|| {
        homedir::my_home().unwrap()
        .expect("Should be able to find the home path")
        .join(LN_CONFIG_DIRECTORY)
    })
}

impl ConfigManager {
    /// Config manager's only use now is to manage nodes in a directory
    /// If the path is None, it will default to the home directory / LN_CONFIG_DIRECTORY
    pub fn new(base_path: Option<PathBuf>) -> Self {
        let path = path_or_default(base_path);
        debug!("Creating config manager at {path:?}");
        Self{path: path}
    }

    /// Adds a new node with new Identity and the given name, returns the path
    /// to the config file, or an error, for example if the node already exists
    pub fn add_config(&self, name: &String) -> Result<PathBuf, Box<dyn std::error::Error>> {
        debug!("Adding config {} to {:?}", &name, self.path);
        if self.node_exists(name) {
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, format!("Node {name} already exists!"))));
        }
        let path = self.get_node_config_path(name);
        Config::new(&name).save(&path)?;
        Ok(path)
    }

    /// Gets the config of a node, returns an error if the node does not exist
    pub fn get_node_config(&self, name: &String) -> Result<Config, Box<dyn std::error::Error>> {
        debug!("Getting config for node {name}");
        if self.node_exists(name) {
            let path = self.get_node_config_path(name);
            return Config::load(path);
        } else {
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, format!("Node {name} does not exist!"))));
        }
    }

    /// Checks if a node exists
    pub fn node_exists(&self, name: &String) -> bool {
        let path = self.path.join(name);
        if path.exists() {
            return true;
        }
        return false;
    }

    /// Gets the path where a node of given name should be stored
    /// Does not care if it exists or not 
    pub fn get_node_path(&self, name: &String) -> PathBuf {
        return self.path.join(name);
    }

    /// Gets the path to the config file of a node
    /// Does not care if it exists or not
    pub fn get_node_config_path(&self, name: &String) -> PathBuf {
        return self.get_node_path(name).join("config.json");
    }
}

impl Config {
    /// Creates a new config struct with a new identity
    /// The name is used to identify the node
    pub fn new(name: &String) -> Self {
        debug!("Creating config struct for node {name}");
        Self{name: name.clone(), identity: Keypair::generate_ed25519()}
    }

    pub fn get_identity(&self) -> Keypair {
        self.identity.clone()
    }

    /// Saves the config to the disk at the given path
    /// Serialization
    pub fn save(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>>{
        debug!("Saving config at {:?}",path);
        if let Some(parent) = path.parent() {
            std::fs::DirBuilder::new().recursive(true).mode(0o711).create(&parent).unwrap();
        } else {
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Parent directory not found")));
        }
        
        let mut file = fs::File::create(&path).unwrap();
        file.set_permissions(fs::Permissions::from_mode(0o600)).unwrap();
        let serializable = self.as_serializable();
        file.write_all(serde_json::to_string(&serializable)?.as_bytes())?;
        debug!("Success in writing the {serializable:?} config to {:?}", path);
        Ok(())
    }

    /// Loads a config from the disk at the given path
    /// Deserialization
    pub fn load(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let file = fs::File::open(&path)?;
        let serializable = serde_json::from_reader::<fs::File, ConfigSerializable>(file)?;
        let config = Self::from_serializable(&serializable);
        debug!("Success in reading {serializable:?} from {path:?}");
        config
    }

    /// Converts the config to a serializable struct
    fn as_serializable(&self) -> ConfigSerializable {
        ConfigSerializable {
            name: self.name.clone(),
            identity: self.identity.to_protobuf_encoding().unwrap(),
        }
    }

    /// Converts a serializable struct to a config
    fn from_serializable(config: &ConfigSerializable) -> Result<Self, Box<dyn std::error::Error>> {
        let id = match Keypair::from_protobuf_encoding(&config.identity) {
            Ok(pair) => pair,
            Err(e) => {
                error!("Error when deserializing config: {e:?}");
                return Err(Box::new(e));
            }
        };
        Ok(Config {
            name: config.name.clone(),
            identity: id,
        })
    }


}