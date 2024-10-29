use serde::{Deserialize, Serialize};

/// Messages that can be sent from the UI to the daemon
#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonRequest {
    NewNode { name: String },
    StartNode { name: String },
}

// TODO Messages that can be sent from the daemon to the UI
