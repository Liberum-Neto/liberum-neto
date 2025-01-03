pub mod codec;
pub mod module;
pub mod node_config;
pub mod parser;
pub mod proto;
pub mod types;

use libp2p::futures::StreamExt;
use node_config::NodeConfig;
use proto::{file::PlainFileObject, *};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tokio_util::io::ReaderStream;
use tracing::{debug, error};
use types::NodeInfo;

use anyhow::Result;
use codec::AsymmetricMessageCodec;
use futures::prelude::*;
use tokio_util::codec::Decoder;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Messages that can be sent from the UI to the daemon
#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonRequest {
    NewNode {
        node_name: String,
        id_seed: Option<String>,
    },
    StartNode {
        node_name: String,
    },
    GetNodeConfig {
        node_name: String,
    },
    OverwriteNodeConfig {
        node_name: String,
        new_cfg: NodeConfig,
    },
    StopNode {
        node_name: String,
    },
    ListNodes,
    GetNodeDetails {
        node_name: String,
    },
    ProvideFile {
        node_name: String,
        path: PathBuf,
    },
    DownloadFile {
        node_name: String,
        id: String,
    },
    GetProviders {
        node_name: String,
        id: String,
    },
    GetPeerId {
        node_name: String,
    },
    Dial {
        node_name: String,
        peer_id: String,
        addr: String,
    },
    PublishFile {
        node_name: String,
        path: PathBuf,
    },
    GetPublishedObjects {
        node_name: String,
    },
    DeleteObject {
        node_name: String,
        object_id: String,
    },
}

/// Messages that are sent from the daemon as a reponse
/// An enum of enums - categorizes the responses
pub type DaemonResult = Result<DaemonResponse, DaemonError>;

#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonResponse {
    NodeCreated,
    NodeStarted,
    NodeConfig(NodeConfig),
    NodeConfigUpdated,
    NodeStopped,
    NodeList(Vec<NodeInfo>),
    NodeDetails(NodeInfo),
    FileProvided {
        id: String,
    },
    Providers {
        ids: Vec<String>,
    },
    FileDownloaded {
        data: PlainFileObject,
    }, // TODO ideally the data should not be a Vec<u8> but some kind of a stream to save it to disk instead of downloading the whole file in memory
    PeerId {
        id: String,
    },
    Dialed,
    FilePublished {
        id: String,
    },
    PublishedObjectsList {
        object_infos: Vec<Hash>,
    },
    ObjectDeleted {
        deleted_myself: bool,
        deleted_count: u32,
        failed_count: u32,
    },
}

/// Errors that can be returned by the daemon
/// An enum of enums - categorizes the errors, just like responses
#[derive(Serialize, Deserialize, Debug, Error)]
pub enum DaemonError {
    #[error("Node already exist: {0}")]
    NodeAlreadyExist(String),
    #[error("Node don't exist: {0}")]
    NodeDoesNotExist(String),
    #[error("Other error: {0}")]
    Other(String),
}

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
pub fn str_to_file_id(s: &str) -> Result<libp2p::kad::RecordKey> {
    let k: Vec<u8> = bs58::decode::<Vec<u8>>(s.into()).into_vec()?;
    let k = libp2p::kad::RecordKey::from(k);
    Ok(k)
}

pub fn file_id_to_str(id: libp2p::kad::RecordKey) -> String {
    bs58::encode(id.to_vec()).into_string()
}

pub fn node_keypair_from_seed(seed: &str) -> libp2p::identity::Keypair {
    let bytes = blake3::hash(seed.as_bytes()).as_bytes().to_vec(); // hash so that the seed can be of any length
    let mut id_buf = [0u8; 32];
    id_buf[..bytes.len()].copy_from_slice(bytes.as_slice()); // copy the bytes to the buffer
    libp2p::identity::Keypair::ed25519_from_bytes(id_buf).unwrap()
}
