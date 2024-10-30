use libp2p::futures::StreamExt;
use tokio::net::UnixListener;
use tracing::{info, warn, debug};

use liberum_core::messages::*;
use liberum_core::codec::AsymmetricMessageCodec;
use anyhow::Result;
use futures::prelude::*;
use tokio_util::codec::Decoder;
use tokio_util::codec::Framed;

/// Used by the core daemon to listen for incoming connections from UI
/// Only one UI connection is possible at a time

async fn handle_message(message: DaemonRequest) -> DaemonResult {
    debug!("Core received a message {message:?}");
    match message {
        DaemonRequest::NewNodes{ names: _names } => {
            //match config_manager.add_config(&name) {
            Ok(DaemonResponse::NodeResponse(NodeResponse::Created))
        },
        DaemonRequest::StartNodes{ names: _names } => {
            //if let Ok(c)= config_manager.get_node_config(&name) {
            Ok(DaemonResponse::NodeResponse(NodeResponse::Started))
        },
        DaemonRequest::StopNodes{ names: _names } => {
            Ok(DaemonResponse::NodeResponse(NodeResponse::Stopped))
        },
        DaemonRequest::ListNodes => {
            Ok(DaemonResponse::NodeResponse(NodeResponse::List(vec![])))
        },
    }
}

async fn handle_connection(
    mut daemon_socket_framed: Framed<
        tokio::net::UnixStream,
        AsymmetricMessageCodec<DaemonResult, DaemonRequest>,
    >, id: u64
) -> Result<()> {
    loop {
        tokio::select! {
            Some(message) = daemon_socket_framed.next() => {
                info!("Received: {message:?} at {id}");
                match message {
                    Ok(message) => {
                        let response = handle_message(message).await;
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
