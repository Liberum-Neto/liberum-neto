use std::path::Path;

use crate::node::BootstrapNode;
use anyhow::Result;
use libp2p::Multiaddr;
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub bootstrap_nodes: Vec<BootstrapNode>,
    pub external_addresses: Vec<Multiaddr>,
}

impl NodeConfig {
    pub fn new(bootstrap_nodes: Vec<BootstrapNode>, external_addresses: Vec<Multiaddr>) -> Self {
        Self {
            bootstrap_nodes,
            external_addresses,
        }
    }

    pub async fn save(&self, path: &Path) -> Result<()> {
        tokio::fs::write(path, serde_json::to_string(&self)?)
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
