use std::error::Error;
use libp2p::futures::StreamExt;
use tracing::{debug, error, info, warn};
use tokio::sync::mpsc;
use tokio::net::UnixStream;
use std::path::PathBuf;

use tokio_util::codec::Decoder;
use futures::prelude::*;
pub mod codec;
pub mod configs;
pub mod messages;
pub mod core_connection;
use messages::DaemonRequest;
use codec::AsymmetricMessageCodec;



/// Function for a CLI or other UI to connecto to the client daemon
/// Returns a sender and receiver for sending and receiving messages
/// from/to the daemon
pub async fn connect(socket_path: PathBuf) -> Result<(mpsc::Sender<DaemonRequest>, mpsc::Receiver<String>), Box<dyn Error>> {
    let socket = UnixStream::connect(&socket_path).await?;
    let encoder: AsymmetricMessageCodec<DaemonRequest, String> = AsymmetricMessageCodec::new();
    let mut daemon_socket = encoder.framed(socket);
    let (daemon_sender,mut daemon_receiver) = mpsc::channel::<DaemonRequest>(16);
    let (ui_sender, mut ui_receiver) = mpsc::channel::<String>(16);
    tokio::spawn (async move {
        loop {
            tokio::select! {
                Some(message) = daemon_receiver.recv() => {
                    daemon_socket.send(message).await.unwrap();
                    let resp = daemon_socket.next().await.unwrap().unwrap();
                    info!("Received: {}", resp);
                    ui_sender.send(resp).await.unwrap();
                }
            };
        }
    });
    Ok((daemon_sender, ui_receiver))
}