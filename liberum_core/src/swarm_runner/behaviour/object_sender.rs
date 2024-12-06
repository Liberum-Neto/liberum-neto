use crate::proto::{self, TypedObject};
use anyhow::{anyhow, Result};
use libp2p::{
    kad,
    request_response::{self, InboundRequestId, OutboundRequestId, ResponseChannel},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tracing::debug;

use super::super::SwarmContext;

///! The module contains the structures and hanlders for the request_response
///! behaviour used to share files

/// An enum that represents anything that can be provided in the network.
/// Should be replaced with an implementation of the OBJECTS

/// A request to the file_share protocol
#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct ObjectSendRequest {
    pub object: proto::TypedObject,
    pub id: proto::Hash,
}

/// A response from the file_share protocol. Should be replaced with a stream
/// in the future, probably using the VAULT and OBJECTS
#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct ObjectResponse {
    pub object: proto::TypedObject,
    pub id: proto::Hash,
}

impl SwarmContext {
    pub(crate) async fn handle_object_sender(
        &mut self,
        event: request_response::Event<ObjectSendRequest, ObjectResponse>,
    ) {
        match event {
            request_response::Event::Message { message, .. } => match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                    ..
                } => {
                    self.handle_object_sender_request(request_id, request, channel)
                        .await
                }

                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    self.handle_object_sender_response(request_id, response)
                        .await
                }
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
    /// Handle a object_send request depending on the type of the data which ID is requested
    async fn handle_object_sender_request(
        &mut self,
        request_id: InboundRequestId,
        request: ObjectSendRequest,
        response_channel: ResponseChannel<ObjectResponse>,
    ) {
        debug!(
            node = self.node_snapshot.name,
            request = format!("{request:?}"),
            "received object sender request!"
        );

        let id: proto::Hash = blake3::hash(request.object.data.as_slice())
            .as_bytes()
            .try_into()
            .unwrap();
        self.behaviour
            .lock()
            .await
            .pending_object_requests
            .insert(id, (request_id, response_channel));
        self.parse_typed(request.object.clone()).await;
    }

    /// Handle a file share response by sending the data to the pending download
    async fn handle_object_sender_response(
        &mut self,
        request_id: OutboundRequestId,
        response: ObjectResponse,
    ) {
        debug!(
            node = self.node_snapshot.name,
            response = format!("{response:?}"),
            "received object sender response!"
        );
        self.behaviour
            .lock()
            .await
            .pending_get_object
            .remove(&request_id)
            .map(|sender| {
                let _ = sender.send(Ok(response.object));
            });
    }
}
