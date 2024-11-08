use libp2p::{kad, request_response};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::debug;

use super::super::SwarmContext;

pub enum SharedResource {
    File { path: PathBuf },
}

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct FileRequest {
    pub id: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct FileResponse {
    pub data: Vec<u8>,
}

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
                    let id = kad::RecordKey::from(request.id.clone());
                    let file = self.behaviour.published.get(&id);
                    if let Some(file) = file {
                        match file {
                            SharedResource::File { path } => {
                                if let Ok(data) = tokio::fs::read(path).await {
                                    self.swarm
                                        .behaviour_mut()
                                        .file_share
                                        .send_response(channel, FileResponse { data })
                                        .expect("Connection to peer to be still open.");
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
