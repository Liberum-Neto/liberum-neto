mod message_handling;
use message_handling::*;
use tokio::net::UnixListener;
use tracing::{info, warn, debug};

use liberum_core::messages::*;
use liberum_core::codec::AsymmetricMessageCodec;
use anyhow::Result;
use futures::prelude::*;
use tokio_util::codec::Decoder;
use tokio_util::codec::Framed;
use kameo::actor::ActorRef;
use crate::node::NodeStore;

/// Used by the core daemon to listen for incoming connections from UI
/// Only one UI connection is possible at a time

async fn handle_message(message: DaemonRequest, context: &mut ConnectionContext) -> DaemonResult {
    debug!("Core received a message {message:?}");
    match message {
        DaemonRequest::NewNodes{ names } => {
            handle_new_nodes(names, context).await
        },
        DaemonRequest::StartNodes{ names } => {
            handle_start_nodes(names, context).await
        },
        DaemonRequest::StopNodes{ names: _names } => {
            handle_stop_nodes(context).await
        },
        DaemonRequest::ListNodes => {
            handle_list_nodes(context).await
        },
    }
}

pub struct ConnectionContext {
    pub node_store: ActorRef<NodeStore>,
}

async fn handle_connection(
    mut daemon_socket_framed: Framed<
        tokio::net::UnixStream,
        AsymmetricMessageCodec<DaemonResult, DaemonRequest>,
    >, id: u64
) -> Result<()> {
    let mut connection_context = ConnectionContext {
        node_store: kameo::spawn(NodeStore::with_default_nodes_dir().await?),
    };
    
    loop {
        tokio::select! {
            Some(message) = daemon_socket_framed.next() => {
                info!("Received: {message:?} at {id}");
                match message {
                    Ok(message) => {
                        let response = handle_message(message, &mut connection_context).await;
                        daemon_socket_framed.send(response).await?;
                    },
                    Err(e) => {warn!("Error receiving message: {e:?}"); break;}
                };
            },
            else => {
                debug!("Connection closed {id}");
                break;
            }
        }
    }
    Ok(())
}
pub async fn listen(
    listener: UnixListener
) -> Result<()> {
    info!("Server listening on {:?}", listener);
    let mut id = 0;
    loop {
        let (daemon_socket, _) = listener.accept().await?;
        info!("Handling a new connection at {id}");
        let daemon_socket_framed: Framed<
            tokio::net::UnixStream,
            AsymmetricMessageCodec<DaemonResult, DaemonRequest>,
        > = AsymmetricMessageCodec::new().framed(daemon_socket);
        tokio::spawn(handle_connection(
            daemon_socket_framed,
            id.clone()
        ));
        id += 1;
    }
}
