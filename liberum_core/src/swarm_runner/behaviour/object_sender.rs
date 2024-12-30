use anyhow::anyhow;
use anyhow::Result;
use liberum_core::parser::{self, ObjectEnum};
use liberum_core::proto::{
    self, file::PlainFileObject, queries::*, signed::SignedObject, ResultObject, TypedObject,
    UUIDTyped,
};
use libp2p::identity::PublicKey;
use libp2p::{
    kad,
    request_response::{self, InboundRequestId, OutboundRequestId, ResponseChannel},
};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::{debug, error};

use crate::vault;

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
        mut response_channel: ResponseChannel<ObjectResponse>,
    ) {
        let mut typed: Option<TypedObject> = Some(obj);
        while let Some(obj) = typed.clone() {
            let obj = parser::parse_typed(obj).await;
            if let Err(e) = obj {
                error!(
                    node = self.node_snapshot.name,
                    err = format!("{e}"),
                    "Error parsing request object"
                );
                self.respond_err(&request, response_channel);
                return;
            }
            let obj = obj.expect("To not be err, as it was checked earlier");
            let resp;
            match obj {
                parser::ObjectEnum::Signed(obj) => {
                    resp = self
                        .handle_request_signed_file(
                            obj,
                            &id,
                            &request,
                            &request_id,
                            response_channel,
                        )
                        .await;
                }
                parser::ObjectEnum::PlainFile(obj) => {
                    resp = self
                        .handle_request_plain_file(
                            obj,
                            &id,
                            &request,
                            &request_id,
                            response_channel,
                        )
                        .await
                }
                parser::ObjectEnum::Query(query) => {
                    resp = self
                        .handle_request_query(query, &id, &request, &request_id, response_channel)
                        .await
                }
                _ => {
                    return;
                }
            }
            if let Some((t, resp_chan)) = resp {
                typed = Some(t);
                response_channel = resp_chan;
            } else {
                return;
            }
        }
    }

    async fn handle_request_signed_file(
        &mut self,
        obj: SignedObject,
        _id: &proto::Hash,
        _request: &ObjectSendRequest,
        _request_id: &InboundRequestId,
        response_channel: ResponseChannel<ObjectResponse>,
    ) -> Option<(TypedObject, ResponseChannel<ObjectResponse>)> {
        // signed parsing
        Some((obj.object, response_channel))
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
    async fn handle_request_plain_file(
        &mut self,
        _obj: PlainFileObject,
        id: &proto::Hash,
        request: &ObjectSendRequest,
        _request_id: &InboundRequestId,
        response_channel: ResponseChannel<ObjectResponse>,
    ) -> Option<(TypedObject, ResponseChannel<ObjectResponse>)> {
        let r = self.put_object_into_vault(request.object.clone()).await;
        if let Err(e) = r {
            error!(
                node = self.node_snapshot.name,
                err = format!("{e}"),
                "Failed to put object into vault"
            );
            self.respond_err(request, response_channel);
            return None;
        }

        let qid = self
            .swarm
            .behaviour_mut()
            .kademlia
            .start_providing(kad::RecordKey::from(id.bytes.to_vec()));

        if let Err(e) = qid {
            error!(
                node = self.node_snapshot.name,
                err = format!("{e}"),
                "Failed to start providing"
            );
            self.respond_err(request, response_channel);
            return None;
        }

        let qid = qid.expect("To not be err, as it was checked earlier");
        self.behaviour
            .pending_outer_start_providing
            .insert(qid, (request.object_id.clone(), response_channel));

        None
    }

    async fn handle_request_query(
        &mut self,
        query: QueryObject,
        id: &proto::Hash,
        request: &ObjectSendRequest,
        request_id: &InboundRequestId,
        response_channel: ResponseChannel<ObjectResponse>,
    ) -> Option<(TypedObject, ResponseChannel<ObjectResponse>)> {
        let typed = query.query_object;
        while let Some(obj) = Some(typed.clone()) {
            let obj = parser::parse_typed(obj).await;
            if let Err(e) = obj {
                debug!(
                    node = self.node_snapshot.name,
                    err = format!("{e}"),
                    "query_object couldn't get parsed"
                );
                self.respond_err(&request, response_channel);
                return None;
            }
            let query = obj.expect("To not be err, as it was checked earlier");

            return match query {
                parser::ObjectEnum::SimpleIDQuery(query) => {
                    self.handle_query_simple_id(query, &id, request, &request_id, response_channel)
                        .await
                }
                parser::ObjectEnum::DeleteObject(delete_object) => {
                    self.handle_query_delete_object(
                        delete_object,
                        &id,
                        request,
                        &request_id,
                        response_channel,
                    )
                    .await
                }
                _ => {
                    error!(
                        node = self.node_snapshot.name,
                        "Query object TypedObject was not a SimpleQueryID {}", query
                    );
                    let _ = self.swarm.behaviour_mut().object_sender.send_response(
                        response_channel,
                        ObjectResponse {
                            object: proto::ResultObject { result: Err(()) }.into(),
                            object_id: request.object_id.clone(),
                        },
                    );
                    None
                }
            };
        }
        None
    }

    async fn handle_query_delete_object(
        &mut self,
        delete_object: DeleteObjectQuery,
        _request_full_object_id: &proto::Hash,
        request: &ObjectSendRequest,
        _request_id: &InboundRequestId,
        response_channel: ResponseChannel<ObjectResponse>,
    ) -> Option<(TypedObject, ResponseChannel<ObjectResponse>)> {
        let obj = self.get_object_from_vault(delete_object.id.clone()).await;
        if let None = obj {
            debug!(
                node = self.node_snapshot.name,
                obj_id = delete_object.id.to_string(),
                "Received Delete Object Query for file not in vault"
            );
            self.respond_err(&request, response_channel);
            return None;
        }

        let obj = parser::parse_typed(obj.unwrap()).await.unwrap();
        if let ObjectEnum::Signed(signed) = obj {
            let key = delete_object.verification_key_ed25519.try_into();
            if let Err(e) = key {
                debug!(
                    node = self.node_snapshot.name,
                    err = format!("{e}"),
                    "Query Delete object received invalid key"
                );
                self.respond_err(&request, response_channel);
                return None;
            }
            let request_public_key: PublicKey = key.unwrap();

            let verified = signed.verify_ed25519(request_public_key);
            if let Ok(verified) = verified {
                if verified {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .stop_providing(&request.object_id.clone().into());
                    self.vault_ref
                        .ask(vault::DeleteTypedObject {
                            hash: delete_object.id,
                        })
                        .await
                        .ok();
                    self.respond_ok(&request, response_channel);
                    return None;
                } else {
                    self.respond_err(&request, response_channel);
                    return None;
                }
            } else {
                self.respond_err(&request, response_channel);
                return None;
            }
        } else {
            self.respond_err(&request, response_channel);
            return None;
        }
    }

    async fn handle_query_simple_id(
        &mut self,
        query: SimpleIDQuery,
        _request_full_object_id: &proto::Hash,
        request: &ObjectSendRequest,
        _request_id: &InboundRequestId,
        response_channel: ResponseChannel<ObjectResponse>,
    ) -> Option<(TypedObject, ResponseChannel<ObjectResponse>)> {
        let obj = self.get_object_from_vault(query.id.clone()).await;
        if let None = obj {
            error!(
                node = self.node_snapshot.name,
                "Failed to get asked object from vault"
            );
            self.respond_err(&request, response_channel);
            return None;
        }
        let obj = obj.expect("To not be err, as it was checked earlier");

        let calculated_obj_id = proto::Hash::try_from(&obj);
        if let Err(e) = calculated_obj_id {
            error!(
                err = format!("{e}"),
                node = self.node_snapshot.name,
                "Failed to calculate hash of object from vault"
            );
            self.respond_err(&request, response_channel);
            return None;
        }
        let calculated_obj_id = calculated_obj_id.expect("Not to be err as it was checked earlier");

        if query.id != calculated_obj_id {
            error!(
                received_obj_id = bs58::encode(&query.id.bytes).into_string(),
                calculated_obj_id = bs58::encode(&calculated_obj_id.bytes).into_string(),
                node = self.node_snapshot.name,
                "Id of object from vault does not match requested & provided ID"
            )
        }

        let _ = self.swarm.behaviour_mut().object_sender.send_response(
            response_channel,
            ObjectResponse {
                object: obj,
                object_id: calculated_obj_id,
            },
        );

        None
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
