use bincode::config;
use libp2p::identity::Keypair;
use homedir;
use serde::{Deserialize, Serialize};
use tracing::{debug,error};
use void::ResultVoidErrExt;
use std::{fs, io::Write, os::unix::fs::{DirBuilderExt, PermissionsExt}, path::{self, Path, PathBuf}, string};
use anyhow::{Result, anyhow};
/// This module manages saving the configuration of the nodes to the disk.


const CONFIG_DIRECTORY_NAME: &str = ".liberum-neto";

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
fn path_or_default(path: Option<PathBuf>) -> Result<PathBuf> {
    match path {
        Some(p) => Ok(p),
        None => {
            let h = homedir::my_home()?.ok_or(anyhow!("Could not find home directory"))?;
            Ok(h.join(CONFIG_DIRECTORY_NAME))
        }
    }
}

impl ConfigManager {
    /// Config manager's only use now is to manage nodes in a directory
    /// If the path is None, it will default to the home directory / LN_CONFIG_DIRECTORY
    pub fn new(base_path: Option<PathBuf>) -> Result<Self> {
        let path = path_or_default(base_path)?;
        debug!("Creating config manager at {path:?}");
        Ok(Self{path})
    }

    /// Adds a new node with new Identity and the given name, returns the path
    /// to the config file, or an error, for example if the node already exists
    pub fn add_config(&self, name: &String) -> Result<PathBuf> {
        debug!("Adding config {} to {:?}", &name, self.path);
        if self.node_exists(name) {
            error!("Node {name} already exists!");
            return Err(anyhow!(format!("Node {name} already exists!")))
        }
        let path = self.get_node_config_path(name);
        Config::new(&name).save(&path)?;
        Ok(path)
    }

    /// Gets the config of a node, returns an error if the node does not exist
    pub fn get_node_config(&self, name: &str) -> Result<Config> {
        debug!("Getting config for node {name}");
        if ! self.node_exists(name) {
            error!("Node {name} does not exist!");
            return Err(anyhow!(format!("Node {name} does not exist!")))
        }
        let path = self.get_node_config_path(name);
        return Config::load(path.as_path());

    }

    pub fn save_node_config(&self, config: &Config) -> Result<()> {
        let path = self.get_node_config_path(&config.name);
        config.save(&path)
    }

    /// Checks if a node exists
    pub fn node_exists(&self, name: &str) -> bool {
        let path = self.path.join(name);
        if path.exists() && path.join("config.json").exists() {
            return true;
        }
        return false;
    }

    /// Gets the path where a node of given name should be stored
    /// Does not care if it exists or not 
    pub fn get_node_path(&self, name: &str) -> PathBuf {
        return self.path.join(name);
    }

    /// Gets the path to the config file of a node
    /// Does not care if it exists or not
    pub fn get_node_config_path(&self, name: &str) -> PathBuf {
        return self.get_node_path(name).join("config.json");
    }
}

impl Config {
    /// Creates a new config struct with a new identity
    /// The name is used to identify the node
    fn new(name: &str) -> Self {
        debug!("Creating config struct for node {name}");
        Self{name: String::from(name), identity: Keypair::generate_ed25519()}
    }

    /// Saves the config to the disk at the given path
    /// Serialization
    fn save(&self, path: &Path) -> Result<()>{
        debug!("Saving config at {:?}",path);
        if let Some(parent) = path.parent() {
            std::fs::DirBuilder::new().recursive(true).mode(0o711).create(&parent)?
        } else {
            error!("Parent directory of {path:?} not found");
            return Err(anyhow!("Parent directory not found"));
        }
        
        let mut file = fs::File::create(&path)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        let serializable: ConfigSerializable = self.try_into()?;
        file.write_all(serde_json::to_string(&serializable).or_else(|e| {
            error!("Could not serialize config: {e}");
            Err(anyhow!(e))
        })?.as_bytes())?;
        debug!("Success in writing the {serializable:?} config to {:?}", path);
        Ok(())
    }

    /// Loads a config from the disk at the given path
    /// Deserialization
    fn load(path: &Path) -> Result<Self> {
        let file = fs::File::open(&path)?;
        let serializable = serde_json::from_reader::<fs::File, ConfigSerializable>(file).or_else(
            |e| {
                error!("Could not read config from {path:?}: {e}");
                Err(anyhow!(e))
            })?;
        let config = Config::try_from(&serializable)?;
        debug!("Success in reading {serializable:?} from {path:?}");
        Ok(config)
    }

}

impl TryFrom<&ConfigSerializable> for Config {
    type Error = anyhow::Error;

    fn try_from(value: &ConfigSerializable) -> Result<Self> {
        let id = Keypair::from_protobuf_encoding(&value.identity)?;
        Ok(Config {
            name: value.name.clone(),
            identity: id,
        })
    }
}
impl TryFrom<&Config> for ConfigSerializable {
    type Error = anyhow::Error;

    fn try_from(value: &Config) -> Result<Self> {
        let id = value.identity.to_protobuf_encoding()?;
        Ok(ConfigSerializable {
            name: value.name.clone(),
            identity: id,
        })
    }
}