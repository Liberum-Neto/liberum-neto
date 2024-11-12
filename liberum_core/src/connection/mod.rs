use std::path::PathBuf;

use crate::node;
use crate::node::manager::GetNode;
use crate::node::manager::IsNodeRunning;
use crate::node::manager::NodeManager;
use crate::node::store::ListNodes;
use crate::node::store::LoadNode;
use crate::node::store::NodeStore;
use crate::node::DialPeer;
use crate::node::DownloadFile;
use crate::node::GetProviders;
use crate::node::Node;
use crate::node::ProvideFile;
use anyhow::Result;
use futures::SinkExt;
use futures::StreamExt;
use kameo::actor::ActorRef;
use kameo::request::MessageSend;
use liberum_core::codec::AsymmetricMessageCodec;
use liberum_core::node_config::NodeConfig;
use liberum_core::types::NodeInfo;
use liberum_core::DaemonError;
use liberum_core::DaemonRequest;
use liberum_core::DaemonResponse;
use liberum_core::DaemonResult;
use libp2p::identity::Keypair;
use tokio::net::UnixListener;
use tokio_util::codec::Decoder;
use tokio_util::codec::Framed;
use tracing::{debug, info, warn};

type SocketFramed =
    Framed<tokio::net::UnixStream, AsymmetricMessageCodec<DaemonResult, DaemonRequest>>;

#[derive(Clone)]
struct AppContext {
    node_manager: ActorRef<NodeManager>,
}

impl AppContext {
    fn new(node_store: ActorRef<NodeStore>) -> Self {
        AppContext {
            node_manager: kameo::spawn(NodeManager::new(node_store.clone())),
        }
    }
}

pub async fn listen(listener: UnixListener) -> Result<()> {
    info!("Server listening on {:?}", listener);
    let mut id = 0;
    let app_context = AppContext::new(kameo::spawn(NodeStore::with_default_nodes_dir().await?));
    loop {
        let (daemon_socket, _) = listener.accept().await?;
        info!(conn_id = id, "Handling a new connection");
        let daemon_socket_framed: SocketFramed =
            AsymmetricMessageCodec::new().framed(daemon_socket);
        tokio::spawn(handle_connection(
            daemon_socket_framed,
            id.clone(),
            app_context.clone(),
        ));
        id = id.wrapping_add(1);
    }
}

async fn handle_connection(
    mut daemon_socket_framed: SocketFramed,
    id: u64,
    app_context: AppContext,
) -> Result<()> {
    loop {
        tokio::select! {
            Some(message) = daemon_socket_framed.next() => {
                debug!("Received: {message:?} at {id}");
                match message {
                    Ok(message) => {
                        let response = handle_message(message, &app_context).await;
                        daemon_socket_framed.send(response).await?;
                    },
                    Err(e) => {warn!(err=e.to_string(), "Error receiving message"); break;}
                };
            },
            else => {
                debug!(conn_id=id, "Connection closed");
                break;
            }
        }
    }
    Ok(())
}

/// Used by the core daemon to listen for incoming connections from UI
/// Only one UI connection is possible at a time
async fn handle_message(message: DaemonRequest, context: &AppContext) -> DaemonResult {
    match message {
        DaemonRequest::NewNode { node_name, id_seed } => {
            handle_new_node(node_name, id_seed, context).await
        }
        DaemonRequest::StartNode { node_name } => handle_start_node(node_name, context).await,
        DaemonRequest::GetNodeConfig { node_name } => {
            handle_get_node_config(node_name, context).await
        }
        DaemonRequest::OverwriteNodeConfig { node_name, new_cfg } => {
            handle_overwrite_node_config(node_name, new_cfg, context).await
        }
        DaemonRequest::StopNode { node_name } => handle_stop_node(node_name, context).await,
        DaemonRequest::ListNodes => handle_list_nodes(context).await,
        DaemonRequest::ProvideFile { node_name, path } => {
            handle_provide_file(&node_name, path, context).await
        }
        DaemonRequest::DownloadFile { node_name, id } => {
            handle_download_file(node_name, id, context).await
        }
        DaemonRequest::GetProviders { node_name, id } => {
            handle_get_providers(node_name, id, context).await
        }
        DaemonRequest::GetPeerId { node_name } => handle_get_peer_id(node_name, context).await,
        DaemonRequest::Dial {
            node_name,
            peer_id,
            addr,
        } => handle_dial(node_name, peer_id, addr, context).await,
        DaemonRequest::PublishFile { node_name, path } => {
            handle_publish_file(node_name, path, context).await
        }
    }
}

async fn handle_get_peer_id(node_name: String, context: &AppContext) -> DaemonResult {
    let node = context
        .node_manager
        .ask(node::manager::GetNode { name: node_name })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to get peer id"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let peer_id = node
        .ask(node::GetPeerId)
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to get peer id"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    Ok(DaemonResponse::PeerId {
        id: peer_id.to_base58(),
    })
}

async fn handle_new_node(
    name: String,
    id_seed: Option<String>,
    context: &AppContext,
) -> DaemonResult {
    let keypair = match id_seed {
        Some(seed) => liberum_core::node_keypair_from_seed(&seed),
        None => Keypair::generate_ed25519(),
    };
    let node = Node::builder()
        .name(name)
        .keypair(keypair)
        .build()
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let resp = context
        .node_manager
        .ask(node::manager::CreateNode { node })
        .send()
        .await;
    match resp {
        Err(e) => Err(DaemonError::Other(e.to_string())),
        Ok(_resp) => Ok(DaemonResponse::NodeCreated),
    }
}

async fn handle_start_node(name: String, context: &AppContext) -> DaemonResult {
    context
        .node_manager
        .ask(node::manager::StartNode { name: name.clone() })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle start node"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    debug!(name = name, "Node started!");

    Ok(DaemonResponse::NodeStarted)
}

async fn handle_get_node_config(name: String, context: &AppContext) -> DaemonResult {
    let config = context
        .node_manager
        .ask(node::manager::GetNodeConfig { name: name.clone() })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle get node config"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    debug!(name = name, "Node config got!");

    Ok(DaemonResponse::NodeConfig(config))
}

async fn handle_overwrite_node_config(
    name: String,
    new_cfg: NodeConfig,
    context: &AppContext,
) -> DaemonResult {
    context
        .node_manager
        .ask(node::manager::OverwriteNodeConfig {
            name: name.clone(),
            new_cfg,
        })
        .send()
        .await
        .inspect_err(|e| {
            debug!(
                err = e.to_string(),
                "Failed to handle overwrite node config"
            )
        })
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    debug!(name = name, "Node config overwritten!");

    Ok(DaemonResponse::NodeConfigUpdated)
}

async fn handle_stop_node(name: String, context: &AppContext) -> DaemonResult {
    let resp = context
        .node_manager
        .ask(node::manager::StopNode { name })
        .send()
        .await
        .map_err(|e| DaemonError::Other(e.to_string()));
    match resp {
        Err(e) => Err(DaemonError::Other(e.to_string())),
        Ok(_nodes) => Ok(DaemonResponse::NodeStopped),
    }
}

async fn handle_list_nodes(context: &AppContext) -> DaemonResult {
    let node_store = context
        .node_manager
        .ask(node::manager::GetNodeStore)
        .send()
        .await
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let all_nodes_names = node_store
        .ask(ListNodes)
        .send()
        .await
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let mut node_infos = Vec::new();

    for name in all_nodes_names.iter() {
        let is_running = context
            .node_manager
            .ask(IsNodeRunning {
                name: name.to_string(),
            })
            .send()
            .await
            .map_err(|e| DaemonError::Other(e.to_string()))?;

        let node = node_store
            .ask(LoadNode {
                name: name.to_string(),
            })
            .send()
            .await
            .map_err(|e| DaemonError::Other(e.to_string()))?;

        let node_ext_addrs = node
            .external_addresses
            .into_iter()
            .map(|addr| addr.to_string())
            .collect::<Vec<String>>();

        let node_info = NodeInfo {
            name: name.to_string(),
            is_running,
            addresses: node_ext_addrs,
        };

        node_infos.push(node_info);
    }

    Ok(DaemonResponse::NodeList(node_infos))
}

async fn handle_provide_file(node_name: &str, path: PathBuf, context: &AppContext) -> DaemonResult {
    let node = context
        .node_manager
        .ask(GetNode {
            name: node_name.to_string(),
        })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle provide file"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let resp_id = node
        .ask(ProvideFile { path })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle provide file"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    Ok(DaemonResponse::FileProvided { id: resp_id })
}

async fn handle_get_providers(node_name: String, id: String, context: &AppContext) -> DaemonResult {
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

// TODO! Downloading a file is blocking now, it should be done in background in some way
async fn handle_download_file(node_name: String, id: String, context: &AppContext) -> DaemonResult {
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

async fn handle_dial(
    node_name: String,
    peer_id: String,
    addr: String,
    context: &AppContext,
) -> DaemonResult {
    let node = context
        .node_manager
        .ask(GetNode {
            name: node_name.to_string(),
        })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle dial"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    node.ask(DialPeer {
        peer_id: peer_id.clone(),
        peer_addr: addr.clone(),
    })
    .send()
    .await
    .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle dial"))
    .map_err(|e| DaemonError::Other(e.to_string()))?;

    debug!("Dialed peer: {}", peer_id);
    Ok(DaemonResponse::Dialed)
}

async fn handle_publish_file(
    node_name: String,
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

    let resp_id = node
        .ask(ProvideFile { path })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle publish file"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    Ok(DaemonResponse::FilePublished { id: resp_id })
}
