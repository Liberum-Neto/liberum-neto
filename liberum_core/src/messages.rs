use std::{path::PathBuf, fmt::Display, fmt, fmt::Formatter};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use strum::Display;


/// Messages that can be sent from the UI to the daemon
#[derive(Serialize, Deserialize, Debug, Display)]
pub enum DaemonRequest {
    NewNodes { names: Vec<String> },
    StartNodes { names: Vec<String> },
    StopNodes { names: Vec<String> },
    ListNodes,
}

/// Messages that are sent from the daemon as a reponse
/// An enum of enums - categorizes the responses
pub type DaemonResult = Result<DaemonResponse, DaemonError>;

#[derive(Serialize, Deserialize, Debug, Display)]
pub enum DaemonResponse {
    NodeResponse(NodeResponse),
}

/// Messages related to nodes
#[derive(Serialize, Deserialize, Debug, Display)]
pub enum NodeResponse {
    NodesCreated,
    NodesStarted,
    NodesStopped,
    ListNodes(Vec<String>),
}

/// Errors that can be returned by the daemon
/// An enum of enums - categorizes the errors, just like responses
#[derive(Serialize, Deserialize, Debug, Display)]
pub enum DaemonError {
    Node(NodeError),
    Other(String)
}
impl std::error::Error for DaemonError {}

/// Errors related to nodes
#[derive(Serialize, Deserialize, Debug, Display)]
pub enum NodeError {
    AlreadyExists(Vec<String>),
    DoesNotExist(Vec<String>),
    Other(String)
}
impl std::error::Error for NodeError {}