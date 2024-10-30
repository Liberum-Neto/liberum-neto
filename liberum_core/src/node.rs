use std::path::{Path, PathBuf};

use anyhow::Result;
use kameo::{message::Message, Actor};
use libp2p::identity::Keypair;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const NODE_DIRECTORY_NAME: &str = ".liberum-neto";

#[derive(Debug)]
pub struct Node {
    name: String,
    keypair: Keypair,
}

impl Node {
    pub fn new(name: &str, keypair: Keypair) -> Self {
        Self {
            name: name.to_string(),
            keypair,
        }
    }

    fn load(node_dir_path: &Path) -> Result<Node> {
        todo!()
    }

    fn save(&self, node_dir_path: &Path) -> Result<()> {
        todo!()
    }
}

impl TryFrom<NodeSerializable> for Node {
    type Error = anyhow::Error;

    fn try_from(value: NodeSerializable) -> Result<Node> {
        todo!()
    }
}

struct LoadNodes(Vec<String>);
struct StoreNodes(Vec<Node>);


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

#[derive(Debug, Actor)]
pub struct NodeStore {
    nodes_dir_path: PathBuf,
}

impl NodeStore {
    pub fn new() -> Result<Self> {
        todo!()
    }

    pub fn with_custom_nodes_dir(custom_nodes_dir_path: &Path) -> Result<Self> {
        todo!()
    }

    fn load_node(&self, name: &str) -> Result<&Node> {
        todo!()
    }

    fn save_node(&self, name: &str) -> Result<()> {
        todo!()
    }

    fn node_dir_exists(&self, name: &str) -> Result<bool> {
        todo!()
    }

    fn resolve_node_dir_path(&self, name: &str) -> &Path {
        todo!()
    }

    fn create_nodes_dir_path(&self) -> Result<()> {
        todo!()
    }

    fn resolve_nodes_dir_path(path_override: Option<&Path>) -> Result<&Path> {
        todo!()
    }
}

impl Message<LoadNodes> for NodeStore {
    type Reply = Result<Option<Node>, NodeStoreError>;

    async fn handle(
            &mut self,
            LoadNodes(names): LoadNodes,
            _: kameo::message::Context<'_, Self, Self::Reply>,
        ) -> Self::Reply {
            Err(NodeStoreError::LoadError { name: "TODO".to_string() })
    }
}

impl Message<StoreNodes> for NodeStore {
    type Reply = Result<(), NodeStoreError>;

    async fn handle(
            &mut self,
            StoreNodes(nodes): StoreNodes,
            _: kameo::message::Context<'_, Self, Self::Reply>,
        ) -> Self::Reply {
            Err(NodeStoreError::StoreError { name: "TODO".to_string() })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct NodeSerializable {
    name: String,
    keypair: Vec<u8>,
}

impl TryFrom<Node> for NodeSerializable {
    type Error = anyhow::Error;

    fn try_from(value: Node) -> Result<NodeSerializable> {
        todo!()
    }
}
