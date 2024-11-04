use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Messages that can be sent from the UI to the daemon
#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonRequest {
    NewNode { name: String },
    StartNode { name: String },
    StopNode { name: String },
    ListNodes,
    PublishFile { node_name: String, path: PathBuf },
}

/// Messages that are sent from the daemon as a reponse
/// An enum of enums - categorizes the responses
pub type DaemonResult = Result<DaemonResponse, DaemonError>;

#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonResponse {
    NodeCreated,
    NodeStarted,
    NodeStopped,
    NodeList(Vec<String>),
    FilePublished { id: String },
}

/// Errors that can be returned by the daemon
/// An enum of enums - categorizes the errors, just like responses
#[derive(Serialize, Deserialize, Debug, Error)]
pub enum DaemonError {
    #[error("Node already exist: {0}")]
    NodeAlreadyExist(String),
    #[error("Node don't exist: {0}")]
    NodeDoesNotExist(String),
    #[error("Other error: {0}")]
    Other(String),
}
