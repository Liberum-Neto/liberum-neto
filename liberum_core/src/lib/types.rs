use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NodeInfo {
    pub name: String,
    pub is_running: bool,
}
