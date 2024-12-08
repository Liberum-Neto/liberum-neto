use anyhow::anyhow;
use liberum_core::parser::{self, ObjectEnum};
use liberum_core::proto::{self, PlainFileObject, QueryObject, TypedObject};
use libp2p::{
    kad,
    request_response::{self, InboundRequestId, OutboundRequestId, ResponseChannel},
};
use serde::{Deserialize, Serialize};
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

        let id = proto::Hash::try_from(&request.object).unwrap();
        if request.object_id != id {
            error!(
                received_id = request.object_id.to_string(),
                id = id.to_string(),
                "File Request ID does not match actual ID!"
            )
        }
        let obj = parser::parse_typed(request.object.clone()).await;
        if let Err(e) = obj {
            error!(err = format!("{e}"), "Error parsing request object");
            self.respond_err(request, response_channel);
            return;
        }
        let obj = obj.unwrap();
        match obj {
            parser::ObjectEnum::PlainFile(obj) => {
                self.handle_request_plain_file(obj, id, request, request_id, response_channel)
                    .await
            }
            parser::ObjectEnum::Query(query) => {
                self.handle_request_query(query, id, request, request_id, response_channel)
                    .await
            }
            _ => {}
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
        if let Some(sender) = self.behaviour.pending_get_object.remove(&request_id) {
            let _ = sender.send(Ok(response.object));
        } else if let Some(sender) = self.behaviour.pending_send_object.remove(&request_id) {
            if let ObjectEnum::Result(r) = parser::parse_typed(response.object).await.unwrap() {
                let _ = sender.send(Ok(r));
            } else {
                let _ = sender.send(Err(anyhow!("Failed to parse result object")));
            }
        }
    }

    fn respond_err(
        &mut self,
        request: ObjectSendRequest,
        response_channel: ResponseChannel<ObjectResponse>,
    ) {
        let _ = self.swarm.behaviour_mut().object_sender.send_response(
            response_channel,
            ObjectResponse {
                object: proto::ResultObject { result: Err(()) }.into(),
                object_id: request.object_id,
            },
        );
    }

    async fn handle_request_plain_file(
        &mut self,
        obj: PlainFileObject,
        id: proto::Hash,
        request: ObjectSendRequest,
        _request_id: InboundRequestId,
        response_channel: ResponseChannel<ObjectResponse>,
    ) {
        self.put_object_into_vault(obj.into()).await.unwrap();
        let qid = self
            .swarm
            .behaviour_mut()
            .kademlia
            .start_providing(kad::RecordKey::from(id.bytes.to_vec()))
            .unwrap();
        self.behaviour
            .pending_object_start_providing
            .insert(qid, (request.object_id, response_channel));
    }

    async fn handle_request_query(
        &mut self,
        query: QueryObject,
        _id: proto::Hash,
        request: ObjectSendRequest,
        _request_id: InboundRequestId,
        response_channel: ResponseChannel<ObjectResponse>,
    ) {
        let query = parser::parse_typed(query.query_object).await;
        if let Err(e) = query {
            debug!(err = format!("{e}"), "query_object couldnt get parsed");
            self.respond_err(request, response_channel);
            return;
        }
        let query = query.unwrap();

        let query = match query {
            parser::ObjectEnum::SimpleIDQuery(query) => parser::ObjectEnum::SimpleIDQuery(query),
            _ => {
                error!("query_object TypedObject is not SimpleIDQuery {query}");
                return;
            }
        };

        match query {
            parser::ObjectEnum::SimpleIDQuery(query) => {
                let obj = self.get_object_from_vault(query.id.clone());
                if let None = obj {
                    self.respond_err(request, response_channel);
                    return;
                }
                let obj = obj.unwrap();
                let id = proto::Hash::try_from(&obj).unwrap();
                if query.id != id {
                    error!(
                        received_id = bs58::encode(&query.id.bytes).into_string(),
                        computed_id = bs58::encode(&id.bytes).into_string(),
                        "ids dont match"
                    )
                }

                let _ = self.swarm.behaviour_mut().object_sender.send_response(
                    response_channel,
                    ObjectResponse {
                        object: obj,
                        object_id: id,
                    },
                );
            }
            _ => {
                // TODO TODO This arm matches, but shouldn't
                error!("Query object TypedObject was not a SimpleQueryID {}", query);
                let _ = self.swarm.behaviour_mut().object_sender.send_response(
                    response_channel,
                    ObjectResponse {
                        object: proto::ResultObject { result: Err(()) }.into(),
                        object_id: request.object_id,
                    },
                );
            }
        }
    }
}
