use bytes::Bytes;
use libp2p::futures::StreamExt;
use std::path::PathBuf;
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tracing::{debug, error};

use futures::prelude::*;
use tokio_util::codec::Decoder;
pub mod codec;
pub mod messages;
use anyhow::Result;
use codec::AsymmetricMessageCodec;
use messages::{DaemonRequest, DaemonResult};
use std::path::Path;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

/// Function for a CLI or other UI to connecto to the client daemon
/// Returns a sender and receiver for sending and receiving messages
/// from/to the daemon
pub async fn connect(
    socket_path: PathBuf,
) -> Result<(mpsc::Sender<DaemonRequest>, mpsc::Receiver<DaemonResult>)> {
    let socket = UnixStream::connect(&socket_path).await?;
    let encoder: AsymmetricMessageCodec<DaemonRequest, DaemonResult> =
        AsymmetricMessageCodec::new();
    let mut daemon_socket = encoder.framed(socket);
    let (daemon_sender, mut daemon_receiver) = mpsc::channel::<DaemonRequest>(16);
    let (ui_sender, ui_receiver) = mpsc::channel::<DaemonResult>(16);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(message) = daemon_receiver.recv() => {
                    match daemon_socket.send(message).await{
                        Err(e)=> error!(err=e.to_string(),"Failed to send message to daemon"),
                        Ok(_) => {}
                    }
                    let resp = match daemon_socket.next().await {
                        Some(Ok(resp)) => resp,
                        Some(Err(e)) => {
                            error!(err=e.to_string(), "Error receiving message");
                            break;
                        },
                        None => {
                            debug!("Connection closed");
                            break;
                        }
                    };
                    match ui_sender.send(resp).await {
                        Err(e) => error!(err=e.to_string(), "Failed to send message to UI"),
                        Ok(_) => {}
                    }
                }
                else => break
            };
        }
    });
    Ok((daemon_sender, ui_receiver))
}

pub async fn get_file_id(path: &Path) -> Result<libp2p::kad::RecordKey> {
    let file = File::open(path).await?;
    let mut stream = ReaderStream::new(file);
    let mut hasher = blake3::Hasher::new();
    while let Some(chunk) = stream.next().await {
        hasher.update(&chunk?);
    }
    let k = *hasher.finalize().as_bytes();
    let k = libp2p::kad::RecordKey::from(k.to_vec());
    Ok(k)
}
pub async fn str_to_file_id(s: &str) -> Result<libp2p::kad::RecordKey> {
    let k: Vec<u8> = bs58::decode::<Vec<u8>>(s.into()).into_vec()?;
    let k = libp2p::kad::RecordKey::from(k);
    Ok(k)
}
pub async fn file_id_to_str(id: libp2p::kad::RecordKey) -> String {
    bs58::encode(id.to_vec()).into_string()
}
