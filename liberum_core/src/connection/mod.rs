use crate::node;
use crate::node::manager::GetNode;
use crate::node::manager::IsNodeRunning;
use crate::node::manager::NodeManager;
use crate::node::store::ListNodes;
use crate::node::store::LoadNode;
use crate::node::store::NodeStore;
use crate::node::DeleteObject;
use crate::node::DialPeer;
use crate::node::DownloadFile;
use crate::node::GetAddresses;
use crate::node::GetProviders;
use crate::node::GetPublishedObjects;
use crate::node::Node;
use crate::node::NodeSnapshot;
use crate::node::ProvideFile;
use crate::node::PublishFile;
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
use libp2p::PeerId;
use std::path::PathBuf;
use tokio::net::UnixListener;
use tokio_util::codec::Decoder;
use tokio_util::codec::Framed;
use tracing::{debug, info, warn};

type SocketFramed =
    Framed<tokio::net::UnixStream, AsymmetricMessageCodec<DaemonResult, DaemonRequest>>;

#[derive(Clone)]
pub struct AppContext {
    node_manager: ActorRef<NodeManager>,
}

impl AppContext {
    pub(super) fn new(node_store: ActorRef<NodeStore>) -> Self {
        AppContext {
            node_manager: kameo::spawn(NodeManager::new(node_store.clone())),
        }
    }
}

pub async fn create_app_context_for_test() -> Result<AppContext, anyhow::Error> {
    Ok(AppContext::new(kameo::spawn(
        NodeStore::with_custom_nodes_dir(std::env::temp_dir().as_path()).await?,
    )))
}

pub async fn listen(listener: UnixListener) -> Result<()> {
    info!("Server listening on {:?}", listener);
    let mut id = 0;
    let app_context = AppContext::new(kameo::spawn(NodeStore::with_default_nodes_dir().await?));
    loop {
        let (daemon_socket, _) = listener.accept().await?;
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
                break;
            }
        }
    }
    Ok(())
}

/// Used by the core daemon to listen for incoming connections from UI
/// Only one UI connection is possible at a time
pub async fn handle_message(message: DaemonRequest, context: &AppContext) -> DaemonResult {
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
        DaemonRequest::GetNodeDetails { node_name } => {
            handle_get_node_details(&node_name, context).await
        }
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
        DaemonRequest::GetPublishedObjects { node_name } => {
            handle_get_published_objects(node_name, context).await
        }
        DaemonRequest::DeleteObject {
            node_name,
            object_id,
        } => handle_delete_object(node_name, object_id, context).await,
    }
}

async fn get_node(node_name: &str, context: &AppContext) -> Result<ActorRef<Node>, DaemonError> {
    context
        .node_manager
        .ask(GetNode {
            name: node_name.to_string(),
        })
        .send()
        .await
        .inspect_err(|e| {
            debug!(
                err = e.to_string(),
                node_name = node_name,
                "Failed to get node"
            )
        })
        .map_err(|e| DaemonError::Other(e.to_string()))
}

async fn handle_get_peer_id(node_name: String, context: &AppContext) -> DaemonResult {
    let node = get_node(&node_name, context).await?;

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
    let node_snapshot = NodeSnapshot::builder()
        .name(name)
        .keypair(keypair)
        .build_snapshot()
        // This can't fail
        .unwrap();

    let resp = context
        .node_manager
        .ask(node::manager::CreateNode { node_snapshot })
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
        let node_info = get_node_details(&name, context)
            .await
            .map_err(|e| DaemonError::Other(e.to_string()))?;
        node_infos.push(node_info);
    }

    Ok(DaemonResponse::NodeList(node_infos))
}

async fn handle_get_node_details(node_name: &str, context: &AppContext) -> DaemonResult {
    let node_info = get_node_details(node_name, context)
        .await
        .map_err(|e| DaemonError::Other(e.to_string()))?;
    DaemonResult::Ok(DaemonResponse::NodeDetails(node_info))
}

async fn get_node_details(node_name: &str, context: &AppContext) -> Result<NodeInfo> {
    let is_running = context
        .node_manager
        .ask(IsNodeRunning {
            name: node_name.to_string(),
        })
        .send()
        .await
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let node_store = context
        .node_manager
        .ask(node::manager::GetNodeStore)
        .send()
        .await
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let node = node_store
        .ask(LoadNode {
            name: node_name.to_string(),
        })
        .send()
        .await
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    let config_ext_addrs = node
        .config
        .external_addresses
        .into_iter()
        .map(|addr| addr.to_string())
        .collect::<Vec<String>>();

    let running_ext_addrs = match is_running {
        true => {
            let node = get_node(node_name, context).await?;

            node.ask(GetAddresses)
                .send()
                .await
                .map_err(|e| DaemonError::Other(e.to_string()))?
        }
        false => Vec::new(),
    };

    let running_ext_addrs = running_ext_addrs
        .into_iter()
        .map(|addr| addr.to_string())
        .collect::<Vec<String>>();

    let node_info = NodeInfo {
        name: node_name.to_string(),
        peer_id: PeerId::from_public_key(&node.keypair.public()).to_string(),
        is_running,
        config_addresses: config_ext_addrs,
        running_addresses: running_ext_addrs,
    };

    Ok(node_info)
}

async fn handle_provide_file(node_name: &str, path: PathBuf, context: &AppContext) -> DaemonResult {
    let node = get_node(&node_name, context).await?;

    let resp_id = node
        .ask(ProvideFile { path })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle provide file"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    Ok(DaemonResponse::FileProvided { id: resp_id })
}

async fn handle_get_providers(node_name: String, id: String, context: &AppContext) -> DaemonResult {
    let node = get_node(&node_name, context).await?;

    let resp = node
        .ask(GetProviders {
            obj_id_str: id.clone(),
        })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to get file providers"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    Ok(DaemonResponse::Providers {
        ids: resp.0.iter().map(|r| r.to_base58()).collect(),
        stats: resp.1,
    })
}

// TODO! Downloading a file is blocking now, it should be done in background in some way
async fn handle_download_file(node_name: String, id: String, context: &AppContext) -> DaemonResult {
    let node = get_node(&node_name, context).await?;

    let resp = node
        .ask(DownloadFile { obj_id_str: id })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle download file"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    Ok(DaemonResponse::FileDownloaded {
        data: resp.0,
        stats: resp.1,
    })
}

async fn handle_dial(
    node_name: String,
    peer_id: String,
    addr: String,
    context: &AppContext,
) -> DaemonResult {
    let node = get_node(&node_name, context).await?;

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
    let node = get_node(&node_name, context).await?;

    let resp_id = node
        .ask(PublishFile { path })
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to handle publish file"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    Ok(DaemonResponse::FilePublished { id: resp_id })
}

async fn handle_get_published_objects(node_name: String, context: &AppContext) -> DaemonResult {
    let node = get_node(&node_name, context).await?;
    let object_infos = node
        .ask(GetPublishedObjects)
        .send()
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to get published objects list"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;
    let object_infos = object_infos.into_iter().map(|id| id.to_string()).collect();
    DaemonResult::Ok(DaemonResponse::PublishedObjectsList { object_infos })
}

async fn handle_delete_object(
    node_name: String,
    object_id: String,
    context: &AppContext,
) -> DaemonResult {
    let node = get_node(&node_name, context).await?;
    let result = node
        .ask(DeleteObject {
            obj_id_str: object_id,
        })
        .await
        .inspect_err(|e| debug!(err = e.to_string(), "Failed to delete object"))
        .map_err(|e| DaemonError::Other(e.to_string()))?;

    DaemonResult::Ok(result)
}
