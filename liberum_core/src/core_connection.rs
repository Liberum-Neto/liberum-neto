use libp2p::futures::StreamExt;
use tracing::{error, info, warn};
use tokio::sync::mpsc;
use tokio::net::UnixListener;

use tokio_util::codec::Decoder;
use futures::prelude::*;
use crate::codec::AsymmetricMessageCodec;
use crate::messages;
use messages::DaemonRequest;
use crate::codec;
use anyhow::{Result, anyhow};
use tokio_util::codec::Framed;

/// Used by the core daemon to listen for incoming connections from UI
/// Only one UI connection is possible at a time
async fn listen_connection(
    daemon_socket_framed: &mut Framed<tokio::net::UnixStream, AsymmetricMessageCodec<String, DaemonRequest>>,
    to_daemon_sender: &mpsc::Sender<DaemonRequest>,
    from_daemon_receiver: &mut mpsc::Receiver<String>) -> Result<()> {
    loop {
        tokio::select! {
            Some(message) = daemon_socket_framed.next() => {
                info!("Received: {message:?}");
                match message {
                    Ok(message) => {
                        to_daemon_sender.send(message).await?;
                        let response = from_daemon_receiver.recv().await.ok_or_else(|| anyhow!("Failed to receive response"))?;
                        daemon_socket_framed.send(response).await?;
                    },
                    Err(e) => {warn!("Error receiving message: {e:?}"); break;}
                };
            },
            else => {
                break;
            }
        }
    }
    Ok(())
}
pub async fn listen(listener: UnixListener, to_daemon_sender: mpsc::Sender<DaemonRequest>, mut from_daemon_receiver: mpsc::Receiver<String>) -> Result<()> {
    info!("Server listening on {:?}", listener);
    
    loop {
        let (daemon_socket, _) = listener.accept().await?;
        info!("Handling a new connection");
        let to_daemon_sender = to_daemon_sender.clone();
        let mut daemon_socket_framed: Framed<tokio::net::UnixStream, AsymmetricMessageCodec<String, DaemonRequest>> = codec::AsymmetricMessageCodec::new().framed(daemon_socket);
        let connection_result=  listen_connection(&mut daemon_socket_framed, &to_daemon_sender, &mut from_daemon_receiver).await;
        match connection_result {
            Err(e) => {error!("Error handling connection: {e:?}");}
            Ok(_) => {},
        }
    }
}