use libp2p::{
    kad,
    request_response::{self, InboundRequestId, OutboundRequestId, ResponseChannel},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::debug;

use super::super::SwarmContext;

///! The module contains the structures and hanlders for the request_response
///! behaviour used to share files

/// An enum that represents anything that can be provided in the network.
/// Should be replaced with an implementation of the OBJECTS
pub enum SharedResource {
    File { path: PathBuf },
}

/// A request to the file_share protocol
#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct FileRequest {
    pub id: Vec<u8>,
}

/// A response from the file_share protocol. Should be replaced with a stream
/// in the future, probably using the VAULT and OBJECTS
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
                    request_id,
                    request,
                    channel,
                    ..
                } => {
                    self.handle_file_share_request(request_id, request, channel)
                        .await
                }

                request_response::Message::Response {
                    request_id,
                    response,
                } => self.handle_file_share_response(request_id, response),
            },
            e => debug!(
                node = self.node.name,
                "Received request_response event! {e:?}"
            ),
        }
    }
}

/// Methods on SwarmContext for handling file sharing
impl SwarmContext {
    /// Handle a file share request depending on the type of the data which ID is requested
    async fn handle_file_share_request(
        &mut self,
        _request_id: InboundRequestId,
        request: FileRequest,
        response_channel: ResponseChannel<FileResponse>,
    ) {
        debug!(node = self.node.name, "Received Request_response request!");
        // Get the file from the providing hashmap
        let id = kad::RecordKey::from(request.id.clone());
        let file = self.behaviour.providing.get(&id);
        // Send the file back to the peer if found
        if let Some(file) = file {
            match file {
                SharedResource::File { path } => {
                    if let Ok(data) = tokio::fs::read(path).await {
                        let r = self
                            .swarm
                            .behaviour_mut()
                            .file_share
                            .send_response(response_channel, FileResponse { data })
                            .inspect_err(|e| {
                                debug!(
                                    node = self.node.name,
                                    "Request_response request response_channel closed: {:?}", e
                                );
                            });
                        if let Err(e) = r {
                            debug!(
                                requested = liberum_core::file_id_to_str(id),
                                node = self.node.name,
                                "Failed to send request_response response: {:?}",
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    /// Handle a file share response by sending the data to the pending download
    fn handle_file_share_response(
        &mut self,
        request_id: OutboundRequestId,
        response: FileResponse,
    ) {
        debug!(node = self.node.name, "received request_response response!");
        // Get the response data and send it to the pending download
        let _ = self
            .behaviour
            .pending_download_file
            .remove(&request_id)
            .expect("Request to still be pending.")
            .send(Ok(response.data));
    }
}
