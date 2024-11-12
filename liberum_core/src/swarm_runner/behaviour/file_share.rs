use libp2p::{kad, request_response};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::debug;

use super::super::SwarmContext;

/// The enum that represents anything that can be provided in the network.
pub enum SharedResource {
    File(FileResource),
}

pub struct FileResource {
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct FileRequest {
    pub id: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct FileResponse {
    pub data: Vec<u8>,
}

/// methods on SwarmContext for handling file sharing
impl SwarmContext {
    pub(crate) async fn handle_file_share(
        &mut self,
        event: request_response::Event<FileRequest, FileResponse>,
    ) {
        match event {
            request_response::Event::Message { message, .. } => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    debug!("Request_response request!");
                    // Get the file from the providing hashmap
                    let id = kad::RecordKey::from(request.id.clone());
                    let file = self.behaviour.providing.get(&id);
                    // Send the file back to the peer if found
                    if let Some(file) = file {
                        match file {
                            SharedResource::File(FileResource { path }) => {
                                if let Ok(data) = tokio::fs::read(path).await {
                                    let r = self
                                        .swarm
                                        .behaviour_mut()
                                        .file_share
                                        .send_response(channel, FileResponse { data })
                                        .inspect_err(|e| {
                                            debug!("Connection closed: {:?}", e);
                                        });
                                    if let Err(e) = r {
                                        debug!(
                                            requested = liberum_core::file_id_to_str(id),
                                            "Failed to send response: {:?}", e
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    debug!("Request_response response!");
                    // Get the response data and send it to the pending download
                    let _ = self
                        .behaviour
                        .pending_download_file
                        .remove(&request_id)
                        .expect("Request to still be pending.")
                        .send(response.data);
                }
            },
            _ => {}
        }
    }
}
