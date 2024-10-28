use std::{error::Error, time::Duration};
use libp2p::futures::StreamExt;
use tracing::{debug, error, info, warn};
use tokio::sync::mpsc;
use tokio::net::UnixListener;

use tokio_util::codec::{Decoder};
use futures::prelude::*;
use crate::messages;
use crate::codec;

/// Used by the core daemon to listen for incoming connections from UI
/// Only one UI connection is possible at a time
pub async fn listen(listener: UnixListener, to_daemon_sender: mpsc::Sender<messages::DaemonRequest>, mut from_daemon_receiver: mpsc::Receiver<String>) {
    info!("Server listening on {:?}", listener);
    
    loop {
        let (daemon_socket, _) = listener.accept().await.unwrap();
        info!("Handling a new connection");
        let to_daemon_sender = to_daemon_sender.clone();
        let mut daemon_socket_framed = codec::AsymmetricMessageCodec::new().framed(daemon_socket);
        loop {
            tokio::select! {
                Some(message) = daemon_socket_framed.next() => {
                    info!("Received: {message:?}");
                    match message {
                        Ok(message) => {
                            to_daemon_sender.send(message).await.unwrap();
                            let response = from_daemon_receiver.recv().await.unwrap();
                            daemon_socket_framed.send(response).await.unwrap();
                        },
                        Err(e) => {warn!("Error receiving message: {e:?}"); break;}
                    };
                },
                else => {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}