use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeInfo {
    pub name: String,
    pub peer_id: String,
    pub is_running: bool,
    pub config_addresses: Vec<String>,
    pub running_addresses: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypedObjectInfo {
    pub id: String,
    pub type_id: Uuid,
}
