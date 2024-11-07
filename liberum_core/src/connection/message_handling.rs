use std::path::PathBuf;

use crate::connection::AppContext;
use crate::node::manager::{CreateNode, GetNode, StartNode, StopNode};
use crate::node::{DownloadFile, GetProviders, Node, PublishFile};
use kameo::request::MessageSend;
use liberum_core::messages::*;
use libp2p::identity::Keypair;
use tracing::debug;

pub async fn handle_new_node(name: String, context: &AppContext) -> DaemonResult {
    let node = Node::builder()
        .name(name)
        .keypair(Keypair::generate_ed25519())
        .build()
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let resp = context.node_manager.ask(CreateNode { node }).send().await;
    match resp {
        Err(e) => Err(DaemonError::Other(e.to_string())),
        Ok(_resp) => Ok(DaemonResponse::NodeCreated),
    }
}

pub async fn handle_start_node(name: String, context: &AppContext) -> DaemonResult {
    context
        .node_manager
        .ask(StartNode { name: name.clone() })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle start node"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    debug!(name = name, "Node started!");

    Ok(DaemonResponse::NodeStarted)
}

pub async fn handle_stop_nodes(name: String, context: &AppContext) -> DaemonResult {
    let resp = context
        .node_manager
        .ask(StopNode { name })
        .send()
        .await
        .map_err(|e| DaemonError::Other(e.to_string()));
    match resp {
        Err(e) => Err(DaemonError::Other(e.to_string())),
        Ok(_nodes) => Ok(DaemonResponse::NodeStopped),
    }
}

pub async fn handle_list_nodes(_context: &AppContext) -> DaemonResult {
    Ok(DaemonResponse::NodeList(vec![]))
}

pub async fn handle_publish_file(
    node_name: &str,
    path: PathBuf,
    context: &AppContext,
) -> DaemonResult {
    let node = context
        .node_manager
        .ask(GetNode {
            name: node_name.to_string(),
        })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle publish file"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let resp = node
        .ask(PublishFile { path })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle publish file"))
        .map_err(|e| DaemonError::Other(e.to_string()));

    match resp {
        Err(e) => Err(DaemonError::Other(e.to_string())),
        Ok(id) => Ok(DaemonResponse::FilePublished { id }),
    }
}

pub async fn handle_get_providers(
    node_name: String,
    id: String,
    context: &AppContext,
) -> DaemonResult {
    let node = context
        .node_manager
        .ask(GetNode {
            name: node_name.clone(),
        })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to get file providers"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let resp = node
        .ask(GetProviders { id: id.clone() })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to get file providers"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;
    Ok(DaemonResponse::Providers {
        ids: resp.iter().map(|r| r.to_base58()).collect(),
    })
}

pub async fn handle_download_file(
    node_name: String,
    id: String,
    context: &AppContext,
) -> DaemonResult {
    let node = context
        .node_manager
        .ask(GetNode {
            name: node_name.to_string(),
        })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle download file"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let file = node
        .ask(DownloadFile { id })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle download file"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    Ok(DaemonResponse::FileDownloaded { data: file })
}
