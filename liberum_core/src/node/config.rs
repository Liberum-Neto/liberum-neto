use crate::node::BootstrapNode;
use libp2p::Multiaddr;
use serde::{Deserialize, Serialize};

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
}
