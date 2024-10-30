use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use liberum_core::configs::ConfigSerializable;
use libp2p::identity::Keypair;
use serde::{Deserialize, Serialize};

const NODE_DIRECTORY_NAME: &str = ".liberum-neto";

#[derive(Debug)]
pub struct Node {
    name: String,
    keypair: Keypair,
}

#[derive(Debug, Serialize, Deserialize)]
struct NodeSerializable {
    name: String,
    keypair: Vec<u8>,
}

#[derive(Debug)]
pub struct NodeStore {
    nodes_dir_path: PathBuf,
    nodes: HashMap<String, Node>,
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

impl NodeStore {
    pub fn new() -> Result<Self> {
        todo!()
    }

    pub fn with_custom_nodes_dir(custom_nodes_dir_path: &Path) -> Result<Self> {
        todo!()
    }

    pub fn add_node(&self, node: Node) -> Result<&mut Node> {
        todo!()
    }

    pub fn get_node(&self, name: &str) -> Result<Option<&Node>> {
        todo!()
    }

    pub fn get_node_mut(&self, name: &str) -> Result<Option<&mut Node>> {
        todo!()
    }

    pub fn get_nodes_names_all(&self) -> Result<Vec<String>> {
        todo!()
    }

    pub fn save_all(&self) -> Result<()> {
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

impl TryFrom<NodeSerializable> for Node {
    type Error = anyhow::Error;

    fn try_from(value: NodeSerializable) -> Result<Node> {
        todo!()
    }
}

impl TryFrom<Node> for NodeSerializable {
    type Error = anyhow::Error;

    fn try_from(value: Node) -> Result<NodeSerializable> {
        todo!()
    }
}
