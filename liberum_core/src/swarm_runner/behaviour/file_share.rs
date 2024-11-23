use anyhow::{anyhow, Result};
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
    pub data: Option<Vec<u8>>,
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
                node = self.node_snapshot.name,
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
        // Get the file from the providing hashmap
        let id = kad::RecordKey::from(request.id.clone());
        let file = self.behaviour.providing.get(&id);
        // Send the file back to the peer if found
        let mut response: FileResponse = FileResponse { data: None };

        // If the file is found, read it and send it back, otherwise None will be sent
        if let Some(file) = file {
            match file {
                SharedResource::File { path } => {
                    if let Ok(data) = tokio::fs::read(path).await {
                        response = FileResponse { data: Some(data) };
                    }
                }
            }
        }

        if response.data.is_none() {
            debug!(
                requested = liberum_core::file_id_to_str(id.clone()),
                node = self.node_snapshot.name,
                "Requested file not found"
            );
        }

        // Send the response
        let r = self
            .swarm
            .behaviour_mut()
            .file_share
            .send_response(response_channel, response)
            .inspect_err(|e| {
                debug!(
                    node = self.node_snapshot.name,
                    "Request_response request response_channel closed: {:?}", e
                );
            });
        if let Err(e) = r {
            debug!(
                requested = liberum_core::file_id_to_str(id),
                node = self.node_snapshot.name,
                "Failed to send request_response response: {:?}",
                e
            );
        }
    }

    /// Handle a file share response by sending the data to the pending download
    fn handle_file_share_response(
        &mut self,
        request_id: OutboundRequestId,
        response: FileResponse,
    ) {
        debug!(node = self.node_snapshot.name, "received request_response response!");
        // Get the response data and send it to the pending download
        let result: Result<Vec<u8>>;
        if let Some(data) = response.data {
            result = Ok(data);
        } else {
            debug!(node = self.node_snapshot.name, "requested File not found");
            result = Err(anyhow!("File not found"));
        }

        let _ = self
            .behaviour
            .pending_download_file
            .remove(&request_id)
            .expect("Request to still be pending.")
            .send(result);
    }
}
