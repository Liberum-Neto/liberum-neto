mod config;
mod store;
use std::path::{self, Path, PathBuf};
use kameo::{message::Message, Actor};
use thiserror::Error;
use tracing::{debug, info, warn, error};
use anyhow::{anyhow, bail, Result};
use config::NodeConfig;
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

pub struct Node {
    pub name: String,
    pub keypair: Keypair,
    pub bootstrap_nodes: Vec<BootstrapNode>,
}

impl Node {
    const CONFIG_FILE_NAME: &'static str = "config.json";
    const KEY_FILE_NAME: &'static str = "keypair";

    pub fn builder() -> NodeBuilder {
        NodeBuilder::default()
    }

    async fn load(node_dir_path: &Path) -> Result<Node> {
        if !node_dir_path.is_dir() {
            error!(
                dir_path = node_dir_path.display().to_string(),
                "node dir path not a directory"
            );
            bail!("node_dir_path is not a directory");
        }

        let config_path = node_dir_path.join(Node::CONFIG_FILE_NAME);
        let config_bytes = tokio::fs::read(config_path)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not read node config from file"))?;
        let config: NodeConfig = serde_json::from_slice(&config_bytes)
            .inspect_err(|e| error!(err = e.to_string(), "could not parse node config JSON"))?;
        let key_path = node_dir_path.join(Node::KEY_FILE_NAME);
        let key_bytes = tokio::fs::read(key_path)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not read node keypair bytes"))?;
        let keypair = Keypair::from_protobuf_encoding(&key_bytes)?;
        let node_name = node_dir_path
            .file_name()
            .ok_or(anyhow!(
                "incorrect node dir path, it should not end with .."
            ))?
            .to_str()
            .ok_or(anyhow!("node dir path is not valid utf-8 string"))
            .inspect_err(|e| error!(err = e.to_string(), "could not resolve node name"))?
            .to_string();
        let node = Node::builder()
            .name(node_name)
            .config(config)
            .keypair(keypair)
            .build()
            .inspect_err(|e| error!(err = e.to_string(), "error while building node"))?;

        Ok(node)
    }

    async fn save(&self, node_dir_path: &Path) -> Result<()> {
        if !node_dir_path.is_dir() {
            warn!("{node_dir_path:?} is not a directory");
            bail!("node_dir_path is not a directory");
        }

        let config: NodeConfig = self.into();
        let config_path = node_dir_path.join(Node::CONFIG_FILE_NAME);
        let key_bytes = self
            .keypair
            .to_protobuf_encoding()
            .inspect_err(|e| error!(err = e.to_string(), "could not convert keypair to bytes"))?;
        let key_path = node_dir_path.join(Node::KEY_FILE_NAME);

        tokio::fs::write(key_path, key_bytes)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not write node keypair"))?;
        tokio::fs::write(config_path, serde_json::to_string(&config)?)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not write node config"))?;

        Ok(())
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("name", &self.name)
            .field("boostrap_nodes", &self.bootstrap_nodes)
            .finish()
    }
}

impl Into<NodeConfig> for &Node {
    fn into(self) -> NodeConfig {
        NodeConfig::new(self.bootstrap_nodes.clone())
    }
}

#[derive(Default)]
pub struct NodeBuilder {
    name: Option<String>,
    keypair: Option<Keypair>,
    bootstrap_nodes: Vec<BootstrapNode>,
}

impl NodeBuilder {
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn keypair(mut self, keypair: Keypair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    pub fn config(mut self, config: NodeConfig) -> Self {
        self.bootstrap_nodes = config.bootstrap_nodes;
        self
    }

    pub fn build(self) -> Result<Node> {
        return Ok(Node {
            name: self.name.ok_or(anyhow!("node name is required"))?,
            keypair: self.keypair.ok_or(anyhow!("keypair is required"))?,
            bootstrap_nodes: self.bootstrap_nodes,
        });
    }
}

pub struct LoadNodes(pub Vec<String>);
pub struct StoreNodes(pub Vec<Node>);


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
        path_override.unwrap_or(Path::new(NodeStore::DEFAULT_NODES_DIRECTORY_NAME));
        if let Some(path_override) = path_override {
            return path_override;
        } else {
            return Path::new(NodeStore::DEFAULT_NODES_DIRECTORY_NAME);
        }
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BootstrapNode {
    #[serde(
        serialize_with = "serialize_peer_id",
        deserialize_with = "deserialize_peer_id"
    )]
    id: PeerId,
    addr: Multiaddr,
}

impl BootstrapNode {
    pub fn new(peer_id: PeerId, addr: Multiaddr) -> Self {
        BootstrapNode { id: peer_id, addr }
    }
}

fn serialize_peer_id<S>(peer_id: &PeerId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let peer_id_bytes = peer_id.to_bytes();
    serializer.serialize_bytes(&peer_id_bytes)
}

fn deserialize_peer_id<'de, D>(deserializer: D) -> Result<PeerId, D::Error>
where
    D: Deserializer<'de>,
{
    let peer_id_bytes = <Vec<u8>>::deserialize(deserializer)?;
    PeerId::from_bytes(&peer_id_bytes)
        .map_err(|e| serde::de::Error::custom(format!("could not deserialize PeerId: {}", e)))
}
