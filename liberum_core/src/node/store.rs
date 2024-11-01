use crate::node::Node;
use anyhow::{anyhow, Result};
use kameo::{message::Message, Actor};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, error, instrument};

struct LoadNodes(Vec<String>);
struct StoreNodes(Vec<Node>);

#[derive(Debug, Actor)]
pub struct NodeStore {
    nodes_dir_path: PathBuf,
}

impl NodeStore {
    const DEFAULT_NODES_DIRECTORY_NAME: &'static str = ".liberum-neto";

    pub async fn new(nodes_dir_path: &Path) -> Result<Self> {
        NodeStore::ensure_nodes_dir_path(nodes_dir_path)
            .await
            .inspect_err(|e| {
                debug!(
                    path = nodes_dir_path.display().to_string(),
                    err = e.to_string(),
                    "failed to ensure nodes dir"
                )
            })?;
        Ok(NodeStore {
            nodes_dir_path: nodes_dir_path.to_path_buf(),
        })
    }

    pub async fn with_default_nodes_dir() -> Result<Self> {
        let nodes_dir_path = NodeStore::resolve_nodes_dir_path(None);
        debug!("creating a node store with a default dir");
        NodeStore::new(nodes_dir_path).await
    }

    pub async fn with_custom_nodes_dir(path: &Path) -> Result<Self> {
        let nodes_dir_path = NodeStore::resolve_nodes_dir_path(Some(path));
        debug!(
            path = nodes_dir_path.display().to_string(),
            "creating a node store with a custom dir"
        );
        NodeStore::new(nodes_dir_path).await
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
        self.nodes_dir_path.join(name)
    }

    async fn ensure_nodes_dir_path(path: &Path) -> Result<()> {
        debug!(path = path.display().to_string(), "ensuring nodes dir");
        tokio::fs::create_dir_all(path).await
            .inspect_err(|e| debug!(err = e.to_string(), "could not create nodes dir"))?;
        Ok(())
    }

    fn resolve_nodes_dir_path(path_override: Option<&Path>) -> &Path {
        path_override.unwrap_or(Path::new(NodeStore::DEFAULT_NODES_DIRECTORY_NAME))
    }
}

impl Message<LoadNodes> for NodeStore {
    type Reply = Result<Vec<Node>, NodeStoreError>;

    #[instrument(skip_all, name = "LoadNodes")]
    async fn handle(
        &mut self,
        LoadNodes(names): LoadNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        let mut result: Vec<Node> = Vec::new();

        for name in names {
            let node = self
                .load_node(&name)
                .await
                .map_err(|_| NodeStoreError::LoadError { name })?;
            result.push(node);
        }

        Ok(result)
    }
}

impl Message<StoreNodes> for NodeStore {
    type Reply = Result<(), NodeStoreError>;

    #[instrument(skip_all, name = "StoreNodes")]
    async fn handle(
        &mut self,
        StoreNodes(nodes): StoreNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        for node in nodes {
            self.save_node(&node)
                .await
                .map_err(|_| NodeStoreError::StoreError { name: node.name })?;
        }

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum NodeStoreError {
    #[error("failed to load node, name: {name}")]
    LoadError { name: String },
    #[error("failed to store node, name: {name}")]
    StoreError { name: String },
}

#[cfg(test)]
mod tests {
    use kameo::request::MessageSend;
    use libp2p::identity::Keypair;
    use tempdir::TempDir;
    use tracing::{span, Instrument, Level};

    use super::*;

    fn init_tracing() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_line_number(true)
            .with_file(true)
            .init();
    }

    #[tokio::test]
    async fn basic_test() {
        init_tracing();
        let tmp_dir = TempDir::new("liberum_tests").unwrap();
        let node_store = NodeStore::with_custom_nodes_dir(tmp_dir.path())
            .instrument(span!(Level::INFO, ""))
            .await
            .unwrap();
        let node_store = kameo::spawn(node_store);
        let new_node = Node::builder()
            .name("test_node".to_string())
            .keypair(Keypair::generate_ed25519())
            .build()
            .unwrap();
        node_store
            .ask(StoreNodes(vec![new_node]))
            .send()
            .await
            .unwrap();
        node_store
            .ask(LoadNodes(vec!["test_node".to_string()]))
            .send()
            .await
            .unwrap();
    }
}
