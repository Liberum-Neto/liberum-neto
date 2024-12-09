use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeInfo {
    pub name: String,
    pub is_running: bool,
    pub addresses: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypedObjectInfo {
    pub id: String,
    pub type_id: Uuid,
}
