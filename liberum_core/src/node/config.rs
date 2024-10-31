use serde::{Serialize, Deserialize};
use crate::node::BootstrapNode;

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub bootstrap_nodes: Vec<BootstrapNode>,
}

impl NodeConfig {
    pub fn new(bootstrap_nodes: Vec<BootstrapNode>) -> Self {
        Self {
            bootstrap_nodes,
        }
    }
}
