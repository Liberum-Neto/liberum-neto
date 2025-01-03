use liberum_core::proto::{
    self, DeleteObjectQuery, QueryObject, ResultObject, SerializablePublicKey, TypedObject,
};
use liberum_core::DaemonQueryStats;
use libp2p::kad::RecordKey;

use crate::swarm_runner::object_sender::ObjectSendRequest;
use crate::vault;

use super::behaviour::object_sender;
use super::SwarmContext;
use anyhow::anyhow;
use anyhow::Result;
use libp2p::swarm::dial_opts::DialOpts;
use libp2p::PeerId;
use libp2p::{kad, Multiaddr};
use std::collections::hash_map;
use tokio::sync::oneshot;
use tracing::error;
use tracing::{debug, info};
pub enum SwarmRunnerError {}

///! The module contains messages that can be sent to the SwarmRunner
///! And the methods to handle them

/// Messages that can be send from a Node actor to the SwarmRunner
pub enum SwarmRunnerMessage {
    /// Echo message, just sends the message back, testing purposes
    Echo {
        message: String,
        response_sender: oneshot::Sender<Result<String, SwarmRunnerError>>,
    },
    /// Dial a peer and remember it as a contact, useful for connecting to other
    /// nodes in the network
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        response_sender: oneshot::Sender<Result<()>>,
    },
    /// Stops the swarm. The node will be informed that the swarm has stopped
    Kill,
    /// Get up to `k` providers for the given key. May return an empty set if
    /// no provider was found.
    GetProviders {
        obj_id: proto::Hash,
        response_sender: oneshot::Sender<(Vec<PeerId>, Option<DaemonQueryStats>)>,
    },
    /// Start providing a file in the network. Only the node that sent this message
    /// will be a provider for the file. The fact of providing the file will be
    /// announced to up to `k` network members close to the provided ID.
    ProvideObject {
        object: TypedObject,
        obj_id: proto::Hash,
        response_sender: oneshot::Sender<Result<()>>,
    },
    /// Download a file from the given node. This requires first finding a provider
    /// using ``GetProviders`` and then sending a request to the provider.
    /// Ok if the file was downloaded successfully, Err otherwise.
    GetObject {
        obj_id: proto::Hash,
        peer_id: PeerId,
        response_sender: oneshot::Sender<Result<TypedObject>>,
    },
    /// Publish a file in the network. This will ask up to `k` nodes near the
    /// published ID to store the file. The nodes will announce to be providers
    /// of the file in the network, just like in `ProvideFile`.
    /// Ok if the Quorum of One provider was reached, Err otherwise.
    ///
    /// The current node will not be a provider of the file as a result. (TODO: Do we want this?)
    SendObject {
        object: TypedObject,
        obj_id: proto::Hash,
        peer_id: PeerId,
        response_sender: oneshot::Sender<Result<ResultObject>>,
    },
    /// Get the `k` closest peers to the given key. The response will contain
    /// up to `k` peers that are closest to the given key.
    GetClosestPeers {
        obj_id: proto::Hash,
        response_sender: oneshot::Sender<Vec<PeerId>>,
    },
    GetAddresses {
        response_sender: oneshot::Sender<Result<Vec<Multiaddr>>>,
    },
    DeleteObject {
        obj_id: proto::Hash,
        peer: PeerId,
        response_sender: oneshot::Sender<Result<ResultObject>>,
    },
    StopProviding {
        obj_id: proto::Hash,
        response_sender: oneshot::Sender<Result<()>>,
    },
}

/// Methods on SwarmContext for handling SwarmRunner messages
/// When sending a message a oneshot sender is added
/// The sender should be used to send the response back to the caller
///
/// The methods here often need to start a query in the swarm. Handling the query
/// requires remembering the query ID and important data like the sender,
/// because the response will come in a different event
/// and the query ID is the only way to match the response to the query event that will come
impl SwarmContext {
    /// Handles a SwarmRunner message received from the Node actor
    /// Returns true if the swarm should be stopped as a result of the message
    pub(crate) async fn handle_swarm_runner_message(
        &mut self,
        message: SwarmRunnerMessage,
    ) -> Result<bool> {
        match message {
            // Echo message, just send the message back, testing purposes
            SwarmRunnerMessage::Echo {
                message,
                response_sender: resp,
            } => {
                debug!(message = message, "Received Echo!");
                let _ = resp.send(Ok(message));
                Ok(false)
            }

            // Dial a peer and remember it as a contact
            SwarmRunnerMessage::Dial {
                peer_id,
                peer_addr,
                response_sender,
            } => {
                let dial_opts = DialOpts::from(peer_addr.clone());

                if let hash_map::Entry::Vacant(entry) = self
                    .behaviour
                    .pending_inner_dial
                    .entry(dial_opts.connection_id())
                {
                    match self.swarm.dial(dial_opts) {
                        Ok(()) => {
                            entry.insert(response_sender);
                        }
                        Err(err) => {
                            let _ = response_sender.send(Err(anyhow!(err)));
                        }
                    }
                } else {
                    debug!("Already dialing {peer_id}")
                }
                Ok(false)
            }

            // Stops the swarm and informs the node
            SwarmRunnerMessage::Kill => Ok(true),

            // Start providing a file in the network
            SwarmRunnerMessage::ProvideObject {
                object,
                obj_id,
                response_sender,
            } => {
                self.provide_object(object, obj_id, response_sender).await;
                Ok(false)
            }

            // Get providers for a file ID
            SwarmRunnerMessage::GetProviders {
                obj_id,
                response_sender,
            } => {
                self.print_neighbours();
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(kad::RecordKey::new(&obj_id.bytes));
                self.behaviour
                    .pending_inner_get_providers
                    .insert(query_id, (Vec::new(), response_sender));
                Ok(false)
            }

            // Download a file from a given peer
            SwarmRunnerMessage::GetObject {
                obj_id,
                peer_id,
                response_sender,
            } => {
                debug!(
                    "Sending a get object request for obj_id {} to peer {}",
                    obj_id.to_string(),
                    peer_id.to_base58()
                );
                // If the local peer
                if &peer_id == self.swarm.local_peer_id() {
                    debug!(
                        peer = peer_id.to_base58(),
                        local = self.swarm.local_peer_id().to_base58(),
                        "Local peer requested object"
                    );
                    // Should be implemented using a VAULT
                    let object = self.get_object_from_vault(obj_id.clone()).await;
                    if let Some(object) = object {
                        let _ = response_sender.send(Ok(object));
                        return Ok(false);
                    } else {
                        let _ = response_sender.send(Err(anyhow!("Object not found")));
                        return Ok(false);
                    }
                } else {
                    // Send a request to the peer
                    let query_obj: TypedObject = proto::QueryObject {
                        query_object: proto::SimpleIDQuery { id: obj_id.clone() }.into(),
                    }
                    .into();
                    let query_obj_id = proto::Hash::try_from(&query_obj).unwrap();
                    let query_id = self.swarm.behaviour_mut().object_sender.send_request(
                        &peer_id,
                        object_sender::ObjectSendRequest {
                            object: query_obj,
                            object_id: query_obj_id,
                        },
                    );

                    self.behaviour
                        .pending_inner_get_object
                        .insert(query_id, response_sender);
                }
                self.print_neighbours();
                Ok(false)
            }
            SwarmRunnerMessage::GetClosestPeers {
                obj_id,
                response_sender,
            } => {
                self.print_neighbours();
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_closest_peers(obj_id.bytes.to_vec());
                self.behaviour
                    .pending_inner_get_closest_peers
                    .insert(query_id, (Vec::new(), response_sender));
                Ok(false)
            }
            SwarmRunnerMessage::SendObject {
                object,
                obj_id,
                peer_id,
                response_sender,
            } => {
                debug!("Sending Object {:?}", object);

                let calculated_obj_id = proto::Hash::try_from(&object).unwrap();

                if calculated_obj_id != obj_id {
                    debug!(
                        node = self.node_snapshot.name,
                        calculated_obj_id = calculated_obj_id.to_string(),
                        obj_id = obj_id.to_string(),
                        "Object ID does not match the hash of the object"
                    );
                    let _ = response_sender.send(Err(anyhow!(
                        "Object ID does not match the hash of the object"
                    )));
                    return Ok(false);
                }

                let request_id = self.swarm.behaviour_mut().object_sender.send_request(
                    &peer_id,
                    object_sender::ObjectSendRequest {
                        object,
                        object_id: obj_id,
                    },
                );

                self.behaviour
                    .pending_inner_send_object
                    .insert(request_id, response_sender);
                Ok(false)
            }

            SwarmRunnerMessage::GetAddresses { response_sender } => {
                debug!("Getting external addresses");

                let addrs = self
                    .swarm
                    .listeners()
                    .map(|a| a.clone())
                    .collect::<Vec<Multiaddr>>();

                let _ = response_sender.send(Ok(addrs));

                Ok(false)
            }

            SwarmRunnerMessage::DeleteObject {
                obj_id,
                peer,
                response_sender,
            } => {
                let key: SerializablePublicKey = self.node_snapshot.keypair.public().into();
                let delete_query = DeleteObjectQuery {
                    id: obj_id,
                    verification_key_ed25519: key,
                };
                let obj: TypedObject = QueryObject {
                    query_object: delete_query.into(),
                }
                .into();
                let query_id = proto::Hash::try_from(&obj)?;

                let request = ObjectSendRequest {
                    object: obj,
                    object_id: query_id,
                };

                let qid = self
                    .swarm
                    .behaviour_mut()
                    .object_sender
                    .send_request(&peer, request);
                self.behaviour
                    .pending_outer_delete_object
                    .insert(qid, response_sender);
                Ok(false)
            }

            SwarmRunnerMessage::StopProviding {
                obj_id,
                response_sender,
            } => {
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .stop_providing(&RecordKey::from(obj_id.bytes.to_vec()));
                let r = self
                    .vault_ref
                    .ask(vault::DeleteTypedObject { hash: obj_id })
                    .await;
                if let Ok(_) = r {
                    response_sender.send(Ok(())).unwrap();
                } else {
                    response_sender
                        .send(Err(anyhow!("Failed to remove from vault")))
                        .unwrap();
                }
                Ok(false)
            }
        }
    }

    pub(crate) async fn provide_object(
        &mut self,
        object: TypedObject,
        obj_id: proto::Hash,
        response_sender: oneshot::Sender<Result<()>>,
    ) {
        let calculated_obj_id = proto::Hash::try_from(&object).unwrap();
        if obj_id != calculated_obj_id {
            error!(
                received_id = bs58::encode(&obj_id.bytes).into_string(),
                computed_id = bs58::encode(&calculated_obj_id.bytes).into_string(),
                "ids don't match"
            );
            let _ = response_sender.send(Err(anyhow!("IDs dont match")));
            return;
        }
        let obj_id_kad = kad::RecordKey::new(&calculated_obj_id.bytes);
        if self.behaviour.providing.contains_key(&calculated_obj_id) {
            info!(
                node = self.node_snapshot.name,
                obj_id = obj_id.to_string(),
                "File is already being provided"
            );
            return;
        }

        // Add the file to the providing list TODO VAULT
        self.behaviour
            .providing
            .insert(calculated_obj_id, object.clone());

        if let Ok(_) = self.put_object_into_vault(object).await {
            // Strat a query to be providing the file ID in kademlia
            let query_id = self
                .swarm
                .behaviour_mut()
                .kademlia
                .start_providing(obj_id_kad.clone())
                .unwrap();
            self.behaviour
                .pending_inner_start_providing
                .insert(query_id, response_sender);
        }
    }
}
