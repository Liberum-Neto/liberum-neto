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
    DownloadFile { node_name: String, id: String },
    GetProviders { node_name: String, id: String },
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
    Providers { ids: Vec<String> },
    FileDownloaded { data: Vec<u8> }, // TODO ideally the data should not be a Vec<u8> but some kind of a stream to save it to disk instead of downloading the whole file in memory
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
