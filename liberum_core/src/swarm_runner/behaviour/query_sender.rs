use anyhow::anyhow;
use liberum_core::proto::{self, ResultObject, TypedObject};
use libp2p::request_response::{self, InboundRequestId, OutboundRequestId, ResponseChannel};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use super::super::SwarmContext;

///! The module contains the structures and hanlders for the request_response
///! behaviour used to share files

/// An enum that represents anything that can be provided in the network.
/// Should be replaced with an implementation of the OBJECTS

/// A request to the file_share protocol
#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct QueryRequest {
    pub object: TypedObject,
    pub object_id: proto::Hash,
}

/// A response from the file_share protocol. Should be replaced with a stream
/// in the future, probably using the VAULT and OBJECTS
#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct QueryResponse {
    pub objects: Vec<(TypedObject, proto::Hash)>,
}

impl SwarmContext {
    pub(crate) async fn handle_query_sender(
        &mut self,
        event: request_response::Event<QueryRequest, QueryResponse>,
    ) {
        match event {
            request_response::Event::Message { message, .. } => match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                    ..
                } => {
                    self.handle_query_request(request_id, request, channel)
                        .await
                }

                request_response::Message::Response {
                    request_id,
                    response,
                } => self.handle_query_response(request_id, response).await,
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
    async fn handle_query_request(
        &mut self,
        request_id: InboundRequestId,
        request: QueryRequest,
        response_channel: ResponseChannel<QueryResponse>,
    ) {
        debug!(
            node = self.node_snapshot.name,
            request = format!("{request:?}"),
            "received query request!"
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
            self.respond_to_query(vec![], response_channel);
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

        self.handle_query_request_typed(
            request.object.clone(),
            id,
            request,
            request_id,
            response_channel,
        )
        .await
    }

    async fn handle_query_request_typed(
        &mut self,
        obj: proto::TypedObject,
        id: proto::Hash,
        _request: QueryRequest,
        request_id: InboundRequestId,
        response_channel: ResponseChannel<QueryResponse>,
    ) {
        match self.modules.query(obj).await {
            Err(e) => {
                error!(
                    err = format!("{e}"),
                    obj_id = id.to_string(),
                    request_id = request_id.to_string(),
                    "Error storing object"
                );
                self.respond_to_query(vec![], response_channel);
            }
            Ok(ids) => {
                let mut objects: Vec<TypedObject> = Vec::new();
                for id in ids {
                    objects.push(self.get_object_from_vault(id.clone()).await.unwrap());
                }
                self.respond_to_query(objects, response_channel);
                // TODO do anything more?
            }
        }
    }

    /// Handle a file share response by sending the data to the pending download
    async fn handle_query_response(
        &mut self,
        request_id: OutboundRequestId,
        response: QueryResponse,
    ) {
        debug!(
            node = self.node_snapshot.name,
            response = format!("{response:?}"),
            "received object sender response!"
        );
        if let Some(sender) = self.behaviour.pending_outbound_queries.remove(&request_id) {
            if response.objects.len() == 0 {
                let _ = sender.send(Err(anyhow!("No objects found for query")));
            } else {
                let _ = sender.send(Ok(response.objects[0].to_owned().0)); // TODO Should send all objects, not just one
            }
        } else if let Some(sender) = self
            .behaviour
            .pending_outer_delete_object
            .remove(&request_id)
        {
            if response.objects.len() > 0 {
                let _ = sender.send(Ok(ResultObject { result: Ok(()) }));
            } else {
                let _ = sender.send(Err(anyhow!("No objects found for query")));
            }
        }
    }

    fn respond_to_query(
        &mut self,
        objects: Vec<TypedObject>,
        response_channel: ResponseChannel<QueryResponse>,
    ) {
        let objects = objects
            .into_iter()
            .map(|obj| {
                let hash = proto::Hash::try_from(&obj).unwrap();
                (obj, hash)
            })
            .collect(); // TODO Decide on how to handle Hash errors

        let response = QueryResponse { objects };
        let _ = self
            .swarm
            .behaviour_mut()
            .query_sender
            .send_response(response_channel, response);
    }
}
