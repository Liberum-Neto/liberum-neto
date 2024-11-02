mod message_handling;
use crate::node;
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

async fn handle_message(message: DaemonRequest, context: &mut ConnectionContext) -> DaemonResult {
    match message {
        DaemonRequest::NewNodes { names } => handle_new_nodes(names, context).await,
        DaemonRequest::StartNodes { names } => handle_start_nodes(names, context).await,
        DaemonRequest::StopNodes { names } => handle_stop_nodes(names, context).await,
        DaemonRequest::ListNodes => handle_list_nodes(context).await,
    }
}

struct ConnectionContext {
    node_manager: ActorRef<NodeManager>,
    node_store: ActorRef<NodeStore>,
}
impl ConnectionContext {
    fn new(node_store: ActorRef<NodeStore>) -> Self {
        ConnectionContext {
            node_manager: kameo::spawn(NodeManager::new(node_store.clone())),
            node_store,
        }
    }
}

async fn handle_connection(
    mut daemon_socket_framed: Framed<
        tokio::net::UnixStream,
        AsymmetricMessageCodec<DaemonResult, DaemonRequest>,
    >,
    id: u64,
) -> Result<()> {
    let mut connection_context =
        ConnectionContext::new(kameo::spawn(NodeStore::with_default_nodes_dir().await?));

    loop {
        tokio::select! {
            Some(message) = daemon_socket_framed.next() => {
                info!("Received: {message:?} at {id}");
                match message {
                    Ok(message) => {
                        let response = handle_message(message, &mut connection_context).await;
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
    loop {
        let (daemon_socket, _) = listener.accept().await?;
        info!(conn_id = id, "Handling a new connection");
        let daemon_socket_framed: Framed<
            tokio::net::UnixStream,
            AsymmetricMessageCodec<DaemonResult, DaemonRequest>,
        > = AsymmetricMessageCodec::new().framed(daemon_socket);
        tokio::spawn(handle_connection(daemon_socket_framed, id.clone()));
        id = id.wrapping_add(1);
    }
}
