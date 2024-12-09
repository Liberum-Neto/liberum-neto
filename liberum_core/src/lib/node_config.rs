use std::{path::Path, str::FromStr};

use anyhow::Result;
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tracing::error;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeConfig {
    pub bootstrap_nodes: Vec<BootstrapNode>,
    pub external_addresses: Vec<Multiaddr>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes: vec![],
            external_addresses: vec![],
        }
    }
}

impl NodeConfig {
    pub fn new(bootstrap_nodes: Vec<BootstrapNode>, external_addresses: Vec<Multiaddr>) -> Self {
        Self {
            bootstrap_nodes,
            external_addresses,
        }
    }

    pub async fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string(&self)?;
        tokio::fs::write(path, content)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not write node config"))?;

        Ok(())
    }

    pub async fn load(path: &Path) -> Result<NodeConfig> {
        let config_bytes = tokio::fs::read(path)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not read node config from file"))?;

        let config: NodeConfig = serde_json::from_slice(&config_bytes)
            .inspect_err(|e| error!(err = e.to_string(), "could not parse node config JSON"))?;

        Ok(config)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BootstrapNode {
    #[serde(
        serialize_with = "serialize_peer_id",
        deserialize_with = "deserialize_peer_id"
    )]
    pub id: PeerId,
    pub addr: Multiaddr,
}

impl BootstrapNode {
    pub fn new(peer_id: PeerId, addr: Multiaddr) -> Self {
        BootstrapNode { id: peer_id, addr }
    }

    pub fn from_strings(peer_id: &str, addr: &str) -> Result<Self> {
        Ok(BootstrapNode {
            id: PeerId::from_str(peer_id)?,
            addr: Multiaddr::from_str(addr)?,
        })
    }
}

fn serialize_peer_id<S>(peer_id: &PeerId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&peer_id.to_base58())
}

fn deserialize_peer_id<'de, D>(deserializer: D) -> Result<PeerId, D::Error>
where
    D: Deserializer<'de>,
{
    let peer_id_base58 = String::deserialize(deserializer)?;
    PeerId::from_str(&peer_id_base58)
        .map_err(|e| serde::de::Error::custom(format!("could not deserialize PeerId: {}", e)))
}
