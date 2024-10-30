use libp2p::futures::StreamExt;
use std::path::PathBuf;
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tracing::{debug, error};

use futures::prelude::*;
use tokio_util::codec::Decoder;
pub mod codec;
pub mod configs;
pub mod messages;
use anyhow::Result;
use codec::AsymmetricMessageCodec;
use messages::{DaemonRequest, DaemonResult};

/// Function for a CLI or other UI to connecto to the client daemon
/// Returns a sender and receiver for sending and receiving messages
/// from/to the daemon
pub async fn connect(
    socket_path: PathBuf,
) -> Result<(mpsc::Sender<DaemonRequest>, mpsc::Receiver<DaemonResult>)> {
    let socket = UnixStream::connect(&socket_path).await?;
    let encoder: AsymmetricMessageCodec<DaemonRequest, DaemonResult> = AsymmetricMessageCodec::new();
    let mut daemon_socket = encoder.framed(socket);
    let (daemon_sender, mut daemon_receiver) = mpsc::channel::<DaemonRequest>(16);
    let (ui_sender, ui_receiver) = mpsc::channel::<DaemonResult>(16);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(message) = daemon_receiver.recv() => {
                    match daemon_socket.send(message).await{
                        Err(e)=> error!("Failed to send message to daemon: {e}"),
                        Ok(_) => {}
                    }
                    let resp = match daemon_socket.next().await {
                        Some(Ok(resp)) => resp,
                        Some(Err(e)) => {
                            error!("Error receiving message: {e}");
                            break;
                        },
                        None => {
                            debug!("Connection closed");
                            break;
                        }
                    };
                    //info!("Received: {:?}", resp);
                    match ui_sender.send(resp).await {
                        Err(e) => error!("Failed to send message to UI: {e}"),
                        Ok(_) => {}
                    }
                }
            };
        }
    });
    Ok((daemon_sender, ui_receiver))
}
