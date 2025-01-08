use std::path::Path;

use super::UUIDTyped;
use anyhow::bail;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlainFileObject {
    pub name: String,
    pub content: Vec<u8>,
}
impl PlainFileObject {
    pub const UUID: Uuid = uuid!("0193a7c0-3ad3-707c-897b-f23b30400c69");
}
impl UUIDTyped for PlainFileObject {
    fn get_type_uuid(&self) -> Uuid {
        PlainFileObject::UUID
    }
}

impl PlainFileObject {
    pub async fn try_from_path(path: &Path) -> Result<Self> {
        let name = {
            let name = path.file_name();
            if let None = name {
                bail!("Invalid filename! {}", path.to_string_lossy())
            }
            let name = name.unwrap().to_str();
            if let None = name {
                bail!("Invalid filename! {},", path.to_string_lossy())
            }
            name.unwrap().to_string()
        };

        Ok(PlainFileObject {
            name,
            content: tokio::fs::read(path).await?,
        })
    }
}
