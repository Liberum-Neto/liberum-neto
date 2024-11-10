use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeInfo {
    pub name: String,
    pub is_running: bool,
    pub addresses: Vec<String>,
}
