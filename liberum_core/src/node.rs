mod config;
mod store;

use anyhow::{anyhow, bail, Result};
use config::NodeConfig;
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, path::Path};

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
            bail!("node_dir_path is not a directory");
        }

        let config_path = node_dir_path.join(Node::CONFIG_FILE_NAME);
        let config_bytes = tokio::fs::read(config_path).await?;
        let config: NodeConfig = serde_json::from_slice(&config_bytes)?;
        let key_path = node_dir_path.join(Node::KEY_FILE_NAME);
        let key_bytes = tokio::fs::read(key_path).await?;
        let keypair = Keypair::from_protobuf_encoding(&key_bytes)?;
        let node_name = node_dir_path
            .file_name()
            .ok_or(anyhow!(
                "incorrect node dir path, it should not end with .."
            ))?
            .to_str()
            .ok_or(anyhow!("node dir path is not valid utf-8 string"))?
            .to_string();
        let node = Node::builder()
            .name(node_name)
            .config(config)
            .keypair(keypair)
            .build()?;

        Ok(node)
    }

    async fn save(&self, node_dir_path: &Path) -> Result<()> {
        if !node_dir_path.is_dir() {
            bail!("node_dir_path is not a directory");
        }

        let config: NodeConfig = self.into();
        let config_path = node_dir_path.join(Node::CONFIG_FILE_NAME);
        let key_bytes = self.keypair.to_protobuf_encoding()?;
        let key_path = node_dir_path.join(Node::KEY_FILE_NAME);

        tokio::fs::write(key_path, key_bytes).await?;
        tokio::fs::write(config_path, serde_json::to_string(&config)?).await?;

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
