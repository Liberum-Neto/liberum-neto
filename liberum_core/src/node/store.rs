use crate::node::Node;
use anyhow::{anyhow, Result};
use kameo::{message::Message, Actor};
use libp2p::{Multiaddr, PeerId};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use thiserror::Error;
use tracing::{debug, error, instrument};

use super::{config::NodeConfig, BootstrapNode};

pub struct LoadNode {
    pub name: String,
}

pub struct StoreNode {
    pub node: Node,
}

pub struct ListNodes;

pub struct UpdateNodeConfig {
    pub name: String,
    pub bootstrap_node_id: String,
    pub bootstrap_node_addr: String,
}

#[derive(Debug, Actor)]
pub struct NodeStore {
    store_dir_path: PathBuf,
}

impl NodeStore {
    const DEFAULT_NODES_DIRECTORY_NAME: &'static str = ".liberum-neto";

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

    async fn load_node(&self, name: &str) -> Result<Node> {
        let node_dir_path = self.resolve_node_dir_path(name);
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
        }

        Node::load(&node_dir_path).await.map_err(|e| anyhow!(e))
    }

    async fn save_node(&self, node: &Node) -> Result<()> {
        let node_dir_path = self.ensure_node_dir_path(&node.name).await?;
        debug!(
            name = node.name,
            path = node_dir_path.display().to_string(),
            "saving node"
        );
        Node::save(node, &node_dir_path)
            .await
            .map_err(|e| anyhow!("{}", e))
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

        node_dir_path.join(name).join(Node::CONFIG_FILE_NAME)
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

impl Message<LoadNode> for NodeStore {
    type Reply = Result<Node, NodeStoreError>;

    #[instrument(skip_all, name = "LoadNodes")]
    async fn handle(
        &mut self,
        LoadNode { name }: LoadNode,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        let node = self
            .load_node(&name)
            .await
            .map_err(|_| NodeStoreError::LoadError { name })?;

        Ok(node)
    }
}

impl Message<StoreNode> for NodeStore {
    type Reply = Result<(), NodeStoreError>;

    #[instrument(skip_all, name = "StoreNodes")]
    async fn handle(
        &mut self,
        StoreNode { node }: StoreNode,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        self.save_node(&node)
            .await
            .map_err(|_| NodeStoreError::StoreError { name: node.name })?;

        Ok(())
    }
}

impl Message<ListNodes> for NodeStore {
    type Reply = Result<Vec<String>, NodeStoreError>;

    #[instrument(skip_all, name = "ListNodes")]
    async fn handle(
        &mut self,
        _: ListNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
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
}

impl Message<UpdateNodeConfig> for NodeStore {
    type Reply = Result<(), NodeStoreError>;

    #[instrument(skip_all, name = "UpdateNodeConfig")]
    async fn handle(
        &mut self,
        msg: UpdateNodeConfig,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        if !self.node_exists(&msg.name) {
            return Err(NodeStoreError::NodeDoesNotExist { name: msg.name });
        }

        let node_config_path = self.resolve_node_config_path(&msg.name);
        let config_bytes = tokio::fs::read(node_config_path.clone())
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not read node config from file"))
            .map_err(|e| NodeStoreError::OtherError {
                name: msg.name.clone(),
                err: e.into(),
            })?;
        let mut new_config: NodeConfig = serde_json::from_slice(&config_bytes)
            .inspect_err(|e| error!(err = e.to_string(), "could not parse node config JSON"))
            .map_err(|e| NodeStoreError::OtherError {
                name: msg.name.clone(),
                err: e.into(),
            })?;

        new_config.bootstrap_nodes = vec![BootstrapNode {
            id: PeerId::from_str(&msg.bootstrap_node_id).unwrap(),
            addr: Multiaddr::from_str(&msg.bootstrap_node_addr).unwrap(),
        }];

        tokio::fs::write(
            node_config_path,
            serde_json::to_string(&new_config).unwrap(),
        )
        .await
        .inspect_err(|e| error!(err = e.to_string(), "could not write node config"))
        .unwrap();

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum NodeStoreError {
    #[error("failed to load node, name: {name}")]
    LoadError { name: String },
    #[error("failed to store node, name: {name}")]
    StoreError { name: String },
    #[error("node does not exist, name: {name}")]
    NodeDoesNotExist { name: String },
    #[error("other error, name: {name}")]
    OtherError { name: String, err: anyhow::Error },
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
        let new_node = Node::builder()
            .name("test_node".to_string())
            .keypair(Keypair::generate_ed25519())
            .build()
            .unwrap();
        node_store
            .ask(StoreNode { node: new_node })
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
