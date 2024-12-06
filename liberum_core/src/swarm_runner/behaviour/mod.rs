pub mod kademlia;
pub mod object_sender;
use anyhow::Result;
use kameo::request;
use std::collections::{HashMap, HashSet};

use libp2p::{
    kad,
    request_response::{self, OutboundRequestId},
    swarm::NetworkBehaviour,
    PeerId,
};
use object_sender::*;
use tokio::sync::oneshot;

use crate::proto::{self, TypedObject};

use super::SwarmContext;

///! The module contains the definition of the behaviour of the network

/// The behaviour of the network
#[derive(NetworkBehaviour)]
pub struct LiberumNetoBehavior {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub object_sender: request_response::cbor::Behaviour<ObjectSendRequest, ObjectResponse>,
}

/// Data required to handle events from the behaviours. Mostly
/// hashmaps to store query IDs and senders for the requests
pub struct BehaviourContext {
    /// A hashmap of resources that are provided by the node. Should be replaced with
    /// an implementation of VAULT
    pub providing: HashMap<proto::Hash, TypedObject>, // TODO VAULT sHOULD REPLACE THIS
    pub pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<Result<()>>>,
    pub pending_send_object: HashMap<OutboundRequestId, oneshot::Sender<Result<()>>>,
    pub pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pub pending_get_object: HashMap<OutboundRequestId, oneshot::Sender<Result<TypedObject>>>,
    pub pending_dial: HashMap<PeerId, oneshot::Sender<Result<()>>>,
    pub pending_get_closest_peers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pub pending_object_requests: HashMap<
        proto::Hash,
        (
            request_response::InboundRequestId,
            request_response::ResponseChannel<ObjectResponse>,
        ),
    >,
}

impl BehaviourContext {
    pub fn new() -> Self {
        BehaviourContext {
            providing: HashMap::new(),
            pending_start_providing: HashMap::new(),
            pending_send_object: HashMap::new(),
            pending_get_providers: HashMap::new(),
            pending_get_object: HashMap::new(),
            pending_dial: HashMap::new(),
            pending_get_closest_peers: HashMap::new(),
            pending_object_requests: HashMap::new(),
        }
    }
}

impl SwarmContext {
    pub(crate) async fn handle_behaviour_event(&mut self, event: LiberumNetoBehaviorEvent) {
        match event {
            LiberumNetoBehaviorEvent::Kademlia(e) => {
                self.handle_kademlia(e).await;
            }
            LiberumNetoBehaviorEvent::ObjectSender(e) => {
                self.handle_object_sender(e).await;
            }
        }
    }
}
