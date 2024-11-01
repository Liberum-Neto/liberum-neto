use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use kameo::{message::Message, Actor};
use thiserror::Error;
use crate::node::Node;

struct LoadNodes(Vec<String>);
struct StoreNodes(Vec<Node>);

#[derive(Debug, Actor)]
pub struct NodeStore {
    nodes_dir_path: PathBuf,
}

impl NodeStore {
    const DEFAULT_NODES_DIRECTORY_NAME: &'static str = ".liberum-neto";

    pub async fn new(nodes_dir_path: &Path) -> Result<Self> {
        NodeStore::ensure_nodes_dir_path(nodes_dir_path).await?;
        Ok(NodeStore { 
            nodes_dir_path: nodes_dir_path.to_path_buf(),
        })
    }

    pub async fn with_default_nodes_dir() -> Result<Self> {
        let nodes_dir_path = NodeStore::resolve_nodes_dir_path(None);
        NodeStore::new(nodes_dir_path).await
    }

    pub async fn with_custom_nodes_dir(path: &Path) -> Result<Self> {
        let nodes_dir_path = NodeStore::resolve_nodes_dir_path(Some(path));
        NodeStore::new(nodes_dir_path).await
    }

    async fn load_node(&self, name: &str) -> Result<Node> {
        let node_dir_path = self.resolve_node_dir_path(name);
        Node::load(&node_dir_path).await.map_err(|e| anyhow!("{}", e))
    }

    async fn save_node(&self, node: &Node) -> Result<()> {
        let node_dir_path = self.resolve_node_dir_path(&node.name);
        self.ensure_node_dir_path(&node.name).await?;
        Node::save(node, &node_dir_path).await.map_err(|e| anyhow!("{}", e))
    }

    fn node_dir_exists(&self, name: &str) -> bool {
        let node_dir_path = self.resolve_node_dir_path(name);
        node_dir_path.exists()
    }

    async fn ensure_node_dir_path(&self, name: &str) -> Result<()> {
        if !self.node_dir_exists(name) {
            let node_dir_path = self.resolve_node_dir_path(name);
            tokio::fs::create_dir(node_dir_path).await?;
        }

        Ok(())
    }

    fn resolve_node_dir_path(&self, name: &str) -> PathBuf {
       self.nodes_dir_path.join(name)
    }

    async fn ensure_nodes_dir_path(path: &Path) -> Result<()> {
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    fn resolve_nodes_dir_path(path_override: Option<&Path>) -> &Path {
        path_override.unwrap_or(Path::new(NodeStore::DEFAULT_NODES_DIRECTORY_NAME))
    }
}

impl Message<LoadNodes> for NodeStore {
    type Reply = Result<Vec<Node>, NodeStoreError>;

    async fn handle(
            &mut self,
            LoadNodes(names): LoadNodes,
            _: kameo::message::Context<'_, Self, Self::Reply>,
        ) -> Self::Reply {
            let mut result: Vec<Node> = Vec::new();
            
            for name in names {
                let node = self.load_node(&name).await.map_err(|_| NodeStoreError::LoadError { name })?;
                result.push(node);
            }

            Ok(result)
    }
}

impl Message<StoreNodes> for NodeStore {
    type Reply = Result<(), NodeStoreError>;

    async fn handle(
            &mut self,
            StoreNodes(nodes): StoreNodes,
            _: kameo::message::Context<'_, Self, Self::Reply>,
        ) -> Self::Reply {
            for node in nodes {
                self.save_node(&node).await.map_err(|_| NodeStoreError::StoreError { name: node.name })?;
            }

            Ok(())
    }
}

#[derive(Error, Debug)]
pub enum NodeStoreError {
    #[error("failed to load node, name: {name}")]
    LoadError{
        name: String,
    },
    #[error("failed to store node, name: {name}")]
    StoreError{
        name: String,
    },
}
