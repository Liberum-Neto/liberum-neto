use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Messages that can be sent from the UI to the daemon
#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonRequest {
    NewNodes { names: Vec<String> },
    StartNodes { names: Vec<String> },
    StopNodes { names: Vec<String> },
    ListNodes,
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
}

/// Errors that can be returned by the daemon
/// An enum of enums - categorizes the errors, just like responses
#[derive(Serialize, Deserialize, Debug, Error)]
pub enum DaemonError {
    #[error("Nodes already exist: {0:?}")]
    NodesAlreadyExist(Vec<String>),
    #[error("Nodes don't exist: {0:?}")]
    NodesDoesNotExist(Vec<String>),
    #[error("Other error: {0}")]
    Other(String),
}
