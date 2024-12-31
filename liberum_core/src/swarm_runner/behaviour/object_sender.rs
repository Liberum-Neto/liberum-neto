use anyhow::anyhow;
use anyhow::Result;
use liberum_core::parser::{self, ObjectEnum};
use liberum_core::proto::{self, ResultObject, TypedObject, UUIDTyped};

use libp2p::request_response::{self, InboundRequestId, OutboundRequestId, ResponseChannel};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::{debug, error};

use super::super::SwarmContext;

///! The module contains the structures and hanlders for the request_response
///! behaviour used to share files

/// An enum that represents anything that can be provided in the network.
/// Should be replaced with an implementation of the OBJECTS

/// A request to the file_share protocol
#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct ObjectSendRequest {
    pub object: TypedObject,
    pub object_id: proto::Hash,
}

/// A response from the file_share protocol. Should be replaced with a stream
/// in the future, probably using the VAULT and OBJECTS
#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct ObjectResponse {
    pub object: TypedObject,
    pub object_id: proto::Hash,
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
            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
            } => {
                error!(
                    node = self.node_snapshot.name,
                    peeer = peer.to_base58(),
                    request_id = format!("{request_id}"),
                    err = format!("{error}"),
                    "Outbound failure"
                );
                if let Some(sender) = self.behaviour.pending_inner_get_object.remove(&request_id) {
                    let _ = sender.send(Err(anyhow!("Outbound failure").context(error)));
                } else if let Some(sender) = self
                    .behaviour
                    .pending_outer_delete_object
                    .remove(&request_id)
                {
                    let _ = sender.send(Err(anyhow!("Outbound failure").context(error)));
                }
            }
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

        let id: std::result::Result<proto::Hash, anyhow::Error> =
            proto::Hash::try_from(&request.object);
        if let Err(e) = id {
            error!(
                node = self.node_snapshot.name,
                received_id = request.object_id.to_string(),
                err = format!("{e}"),
                "Can't hash received object to verify hash"
            );
            self.respond_err(&request, response_channel);
            return;
        }
        let id = id.expect("To not be err, as it was checked earlier");

        if request.object_id != id {
            error!(
                node = self.node_snapshot.name,
                received_id = request.object_id.to_string(),
                id = id.to_string(),
                "File Request ID does not match actual ID!"
            )
        }

        self.handle_request_typed(
            request.object.clone(),
            id,
            request,
            request_id,
            response_channel,
        )
        .await
    }

    async fn handle_request_typed(
        &mut self,
        obj: proto::TypedObject,
        id: proto::Hash,
        request: ObjectSendRequest,
        request_id: InboundRequestId,
        response_channel: ResponseChannel<ObjectResponse>,
    ) {
        match self.modules.store(obj).await {
            Err(e) => {
                error!(
                    err = format!("{e}"),
                    obj_id = id.to_string(),
                    request_id = request_id.to_string(),
                    "Error storing object"
                );
                self.respond_err(&request, response_channel);
            }
            Ok(_b) => {
                self.respond_ok(&request, response_channel);
                // TODO do anything more?
            }
        }
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
        if let Some(sender) = self.behaviour.pending_inner_get_object.remove(&request_id) {
            let _ = sender.send(Ok(response.object));
        } else if let Some(sender) = self.behaviour.pending_inner_send_object.remove(&request_id) {
            self.send_result_object(response.object, sender).await;
        } else if let Some(sender) = self
            .behaviour
            .pending_outer_delete_object
            .remove(&request_id)
        {
            self.send_result_object(response.object, sender).await;
        }
    }

    fn respond_err(
        &mut self,
        request: &ObjectSendRequest,
        response_channel: ResponseChannel<ObjectResponse>,
    ) {
        let _ = self.swarm.behaviour_mut().object_sender.send_response(
            response_channel,
            ObjectResponse {
                object: proto::ResultObject { result: Err(()) }.into(),
                object_id: request.object_id.clone(),
            },
        );
    }
    fn respond_ok(
        &mut self,
        request: &ObjectSendRequest,
        response_channel: ResponseChannel<ObjectResponse>,
    ) {
        let _ = self.swarm.behaviour_mut().object_sender.send_response(
            response_channel,
            ObjectResponse {
                object: proto::ResultObject { result: Ok(()) }.into(),
                object_id: request.object_id.clone(),
            },
        );
    }

    async fn send_result_object(
        &mut self,
        object: TypedObject,
        sender: oneshot::Sender<Result<ResultObject>>,
    ) {
        let r = parser::parse_typed(object).await;
        match r {
            Ok(obj) => {
                if let ObjectEnum::Result(obj) = obj {
                    let _ = sender.send(Ok(obj));
                } else {
                    let _ = sender.send(Err(anyhow!(
                        "Unsupported object type {}",
                        obj.get_type_uuid()
                    )));
                }
            }
            Err(e) => {
                let _ = sender.send(Err(anyhow!("Failed to parse result object").context(e)));
            }
        }
    }
}
