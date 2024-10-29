use std::{path::PathBuf, fmt::Display, fmt, fmt::Formatter};
use serde::{Deserialize, Serialize};
use anyhow::{Error, Result};


/// Messages that can be sent from the UI to the daemon
#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonRequest {
    NewNodes { names: Vec<String> },
    StartNodes { names: Vec<String> },
    StopNodes { names: Vec<String> },
    ListNodes,
    ListNodePaths { names: Vec<String> },
}

/// Messages that are sent from the daemon as a reponse
/// An enum of enums - categorizes the responses
#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonResponse {
    NodeResponse(NodeResponse),
}

/// Messages related to nodes
#[derive(Serialize, Deserialize, Debug)]
pub enum NodeResponse {
    NodesCreated(Result<(),NodeError>),
    NodesStarted(Result<(),NodeError>),
    NodesStopped(Result<(), NodeError>),
    ListNodes(Vec<String>),
    NodesPaths(Vec<PathBuf>),
}

/// Errors that can be returned by the daemon
/// An enum of enums - categorizes the errors, just like responses
#[derive(Serialize, Deserialize, Debug, strum::Display)]
pub enum DaemonError {
    NodeError(NodeError),
}
impl std::error::Error for DaemonError {}

/// Errors related to nodes
#[derive(Serialize, Deserialize, Debug, strum::Display)]
pub enum NodeError {
    NodeAlreadyExists(Vec<String>),
    NodeDoesNotExist(Vec<String>),
}
impl std::error::Error for NodeError {}