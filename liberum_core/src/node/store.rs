use anyhow::{anyhow, Context, Result};
use kameo::{messages, Actor};
use liberum_core::node_config::NodeConfig;
use libp2p::identity::Keypair;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, error};

use crate::vault::Vault;

use super::NodeSnapshot;

pub struct UpdateNodeConfig {
    pub name: String,
    pub bootstrap_node_id: String,
    pub bootstrap_node_addr: String,
}

#[derive(Debug, Actor)]
pub struct NodeStore {
    store_dir_path: PathBuf,
}

#[derive(Error, Debug)]
pub enum NodeStoreError {
    #[error("node does not exist")]
    NodeDoesNotExist,
    #[error("other error: {err}")]
    OtherError {
        #[from]
        err: anyhow::Error,
    },
}

#[messages]
impl NodeStore {
    #[message]
    pub async fn load_node(&self, name: String) -> Result<NodeSnapshot, NodeStoreError> {
        let node_dir_path = self.resolve_node_dir_path(&name);
        debug!(
            name = name,
            path = node_dir_path.display().to_string(),
            "loading node"
        );

        if !node_dir_path.exists() {
            debug!(
                path = node_dir_path.display().to_string(),
                name = name,
                "node does not exist"
            );

            return Err(NodeStoreError::NodeDoesNotExist);
        }

        if !node_dir_path.is_dir() {
            error!(
                dir_path = node_dir_path.display().to_string(),
                "node dir path not a directory"
            );

            return Err(anyhow!("node_dir_path is not a directory").into());
        }

        let config_path = node_dir_path.join(Self::NODE_CONFIG_FILE_NAME);
        let config = NodeConfig::load(&config_path)
            .await
            .context("failed to load node config")?;
        let key_path = node_dir_path.join(Self::NODE_KEY_FILE_NAME);
        let key_bytes = tokio::fs::read(key_path)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not read node keypair bytes"))
            .context("could not read node keypair bytes")?;
        let keypair = Keypair::from_protobuf_encoding(&key_bytes)
            .context("could not read keypair from protobuf encoded bytes")?;
        let node_snapshot = NodeSnapshot::builder()
            .name(name)
            .keypair(keypair)
            .config(config)
            .build_snapshot()
            // This can't fail
            .unwrap();

        Ok(node_snapshot)
    }

    #[message]
    pub async fn store_node(&self, node_snapshot: NodeSnapshot) -> Result<(), NodeStoreError> {
        let node_dir_path = self
            .ensure_node_dir_path(&node_snapshot.name)
            .await
            .context("could not ensure node dir path")?;

        debug!(
            name = node_snapshot.name,
            path = node_dir_path.display().to_string(),
            "saving node"
        );

        if !node_dir_path.is_dir() {
            error!("node dir path is not a directory");
            return Err(anyhow!("node dir path is not a directory").into());
        }

        let config: NodeConfig = (&node_snapshot).into();
        let config_path = node_dir_path.join(Self::NODE_CONFIG_FILE_NAME);
        let key_bytes = node_snapshot
            .keypair
            .to_protobuf_encoding()
            .inspect_err(|e| error!(err = e.to_string(), "could not convert keypair to bytes"))
            .context("could not convert keypair to bytes")?;
        let key_path = node_dir_path.join(Self::NODE_KEY_FILE_NAME);

        tokio::fs::write(key_path, key_bytes)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not write node keypair"))
            .context("could not write node keypair")?;

        config.save(&config_path).await?;

        Ok(())
    }

    #[message]
    pub async fn get_node_config(&self, name: String) -> Result<NodeConfig, NodeStoreError> {
        if !self.node_exists(&name) {
            return Err(NodeStoreError::NodeDoesNotExist);
        }

        let node_conf_path = self.resolve_node_config_path(&name);
        debug!(
            path = node_conf_path.display().to_string(),
            "Getting current node config"
        );

        let config = NodeConfig::load(&node_conf_path)
            .await
            .map_err(|err| NodeStoreError::OtherError { err })?;

        Ok(config)
    }

    #[message]
    pub async fn overwrite_node_config(
        &self,
        name: String,
        new_cfg: NodeConfig,
    ) -> Result<(), NodeStoreError> {
        if !self.node_exists(&name) {
            return Err(NodeStoreError::NodeDoesNotExist);
        }

        let node_conf_path = self.resolve_node_config_path(&name);

        new_cfg
            .save(&node_conf_path)
            .await
            .map_err(|err| NodeStoreError::OtherError { err })?;

        Ok(())
    }

    #[message]
    pub async fn list_nodes(&self) -> Result<Vec<String>, NodeStoreError> {
        let mut names = Vec::new();
        let mut dir = tokio::fs::read_dir(&self.store_dir_path).await.unwrap();
        while let Some(dir) = dir.next_entry().await.unwrap() {
            if dir.path().is_dir() {
                if let Some(name) = dir.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
        }

        Ok(names)
    }

    #[message]
    pub async fn get_node_vault(&self, name: String) -> Result<Vault> {
        Vault::new_on_disk(&self.resolve_node_dir_path(&name)).await
    }
}

impl NodeStore {
    const DEFAULT_NODES_DIRECTORY_NAME: &'static str = ".liberum-neto";
    const NODE_CONFIG_FILE_NAME: &'static str = "config.json";
    const NODE_KEY_FILE_NAME: &'static str = "keypair";

    pub async fn new(store_dir_path: &Path) -> Result<Self> {
        NodeStore::ensure_store_dir_path(store_dir_path)
            .await
            .inspect_err(|e| {
                error!(
                    path = store_dir_path.display().to_string(),
                    err = e.to_string(),
                    "failed to ensure store dir"
                )
            })?;
        Ok(NodeStore {
            store_dir_path: store_dir_path.to_path_buf(),
        })
    }

    pub async fn with_default_nodes_dir() -> Result<Self> {
        let store_dir_path = NodeStore::resolve_store_dir_path(None)
            .inspect_err(|e| error!(err = e.to_string(), "could not resolve store dir path"))?;
        debug!("creating a node store with a default dir");
        NodeStore::new(&store_dir_path)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not create a node store"))
    }

    pub async fn with_custom_nodes_dir(path: &Path) -> Result<Self> {
        let store_dir_path = NodeStore::resolve_store_dir_path(Some(path))
            .inspect_err(|e| error!(err = e.to_string(), "could not resolve store dir path"))?;
        debug!(
            path = &store_dir_path.display().to_string(),
            "creating a node store with a custom dir"
        );
        NodeStore::new(&store_dir_path)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not create a node store"))
    }

    fn node_exists(&self, name: &str) -> bool {
        let node_dir_path = self.resolve_node_dir_path(&name);
        node_dir_path.exists()
    }

    async fn ensure_node_dir_path(&self, name: &str) -> Result<PathBuf> {
        let node_dir_path = self.resolve_node_dir_path(name);
        debug!(
            name = name,
            path = node_dir_path.display().to_string(),
            "ensuring node dir"
        );

        if !node_dir_path.exists() {
            let node_dir_path = self.resolve_node_dir_path(name);
            tokio::fs::create_dir(node_dir_path).await?;
        }

        Ok(node_dir_path)
    }

    fn resolve_node_dir_path(&self, name: &str) -> PathBuf {
        self.store_dir_path.join(name)
    }

    fn resolve_node_config_path(&self, name: &str) -> PathBuf {
        let node_dir_path = self.resolve_node_dir_path(name);

        node_dir_path.join(Self::NODE_CONFIG_FILE_NAME)
    }

    async fn ensure_store_dir_path(path: &Path) -> Result<()> {
        debug!(path = path.display().to_string(), "ensuring store dir");
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    fn resolve_store_dir_path(path_override: Option<&Path>) -> Result<PathBuf> {
        let home_dir_path = homedir::my_home()?.ok_or(anyhow!("no home directory"))?;
        let store_dir_name =
            path_override.unwrap_or(Path::new(NodeStore::DEFAULT_NODES_DIRECTORY_NAME));
        Ok(home_dir_path.join(store_dir_name))
    }
}

#[cfg(test)]
mod tests {
    use kameo::request::MessageSend;
    use libp2p::identity::Keypair;
    use tempdir::TempDir;

    use super::*;

    #[tokio::test]
    async fn basic_test() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let node_store = NodeStore::with_custom_nodes_dir(tmp_dir.path())
            .await
            .unwrap();
        let node_store = kameo::spawn(node_store);
        let node_snapshot = NodeSnapshot::builder()
            .name("test_node".to_string())
            .keypair(Keypair::generate_ed25519())
            .build_snapshot()
            .unwrap();

        node_store
            .ask(StoreNode { node_snapshot })
            .send()
            .await
            .unwrap();

        let got_node_name = node_store
            .ask(LoadNode {
                name: "test_node".to_string(),
            })
            .send()
            .await
            .unwrap()
            .name;

        assert_eq!(got_node_name, "test_node");
    }

    #[tokio::test]
    #[should_panic]
    async fn test_not_directory() {
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let non_dir_path = tmp_dir.path().join("test_file");

        tokio::fs::write(&non_dir_path, "abc").await.unwrap();

        NodeStore::with_custom_nodes_dir(&non_dir_path)
            .await
            .inspect(|_| panic!("passing non-dir path should not be possible"))
            .unwrap();
    }
}
