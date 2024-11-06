mod message_handling;
use crate::node::manager::NodeManager;
use crate::node::store::NodeStore;
use anyhow::Result;
use futures::prelude::*;
use kameo::actor::ActorRef;
use liberum_core::codec::AsymmetricMessageCodec;
use liberum_core::messages::*;
use message_handling::*;
use tokio::net::UnixListener;
use tokio_util::codec::Decoder;
use tokio_util::codec::Framed;
use tracing::{debug, info, warn};

/// Used by the core daemon to listen for incoming connections from UI
/// Only one UI connection is possible at a time

async fn handle_message(message: DaemonRequest, context: &AppContext) -> DaemonResult {
    match message {
        DaemonRequest::NewNode { name } => handle_new_node(name, context).await,
        DaemonRequest::StartNode { name } => handle_start_node(name, context).await,
        DaemonRequest::UpdateNodeConfig {
            name,
            bootstrap_node_id,
            bootstrap_node_addr,
        } => handle_update_node_config(name, bootstrap_node_id, bootstrap_node_addr, context).await,
        DaemonRequest::StopNode { name } => handle_stop_nodes(name, context).await,
        DaemonRequest::ListNodes => handle_list_nodes(context).await,
        DaemonRequest::PublishFile { node_name, path } => {
            handle_publish_file(&node_name, path, context).await
        }
        DaemonRequest::DownloadFile { node_name, id } => {
            handle_download_file(node_name, id, context).await
        }
        DaemonRequest::GetProviders { node_name, id } => {
            handle_get_providers(node_name, id, context).await
        }
    }
}

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

async fn handle_connection(
    mut daemon_socket_framed: Framed<
        tokio::net::UnixStream,
        AsymmetricMessageCodec<DaemonResult, DaemonRequest>,
    >,
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

pub async fn listen(listener: UnixListener) -> Result<()> {
    info!("Server listening on {:?}", listener);
    let mut id = 0;
    let app_context = AppContext::new(kameo::spawn(NodeStore::with_default_nodes_dir().await?));
    loop {
        let (daemon_socket, _) = listener.accept().await?;
        debug!(conn_id = id, "Handling a new connection");
        let daemon_socket_framed: Framed<
            tokio::net::UnixStream,
            AsymmetricMessageCodec<DaemonResult, DaemonRequest>,
        > = AsymmetricMessageCodec::new().framed(daemon_socket);
        tokio::spawn(handle_connection(
            daemon_socket_framed,
            id.clone(),
            app_context.clone(),
        ));
        id = id.wrapping_add(1);
    }
}
