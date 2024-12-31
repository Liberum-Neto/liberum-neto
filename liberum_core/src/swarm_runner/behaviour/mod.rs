pub mod kademlia;
pub mod object_sender;
pub mod query_sender;
use anyhow::Result;
use liberum_core::{proto::*, DaemonQueryStats};
use libp2p::request_response::ResponseChannel;
use std::collections::HashMap;

use libp2p::{
    kad,
    request_response::{self, OutboundRequestId},
    swarm::{ConnectionId, NetworkBehaviour},
    PeerId,
};
use object_sender::*;
use query_sender::*;
use tokio::sync::oneshot;

use liberum_core::proto::{self, TypedObject};

use super::SwarmContext;

///! The module contains the definition of the behaviour of the network

/// The behaviour of the network
#[derive(NetworkBehaviour)]
pub struct LiberumNetoBehavior {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub object_sender: request_response::cbor::Behaviour<ObjectSendRequest, ObjectResponse>,
    pub query_sender: request_response::cbor::Behaviour<QueryRequest, QueryResponse>,
}

/// Data required to handle events from the behaviours. Mostly
/// hashmaps to store query IDs and senders for the requests
pub struct BehaviourContext {
    /// A hashmap of resources that are provided by the node. Should be replaced with
    /// an implementation of VAULT
    pub providing: HashMap<proto::Hash, TypedObject>, // TODO VAULT sHOULD REPLACE THIS
    pub pending_inner_start_providing: HashMap<kad::QueryId, oneshot::Sender<Result<()>>>,
    pub pending_inner_send_object:
        HashMap<OutboundRequestId, oneshot::Sender<Result<ResultObject>>>,
    pub pending_inner_get_providers: HashMap<
        kad::QueryId,
        (
            Vec<PeerId>,
            oneshot::Sender<(Vec<PeerId>, Option<DaemonQueryStats>)>,
        ),
    >,
    pub pending_inner_get_object: HashMap<OutboundRequestId, oneshot::Sender<Result<TypedObject>>>,
    pub pending_inner_dial: HashMap<ConnectionId, oneshot::Sender<Result<()>>>,
    pub pending_inner_get_closest_peers:
        HashMap<kad::QueryId, (Vec<PeerId>, oneshot::Sender<Vec<PeerId>>)>,
    pub pending_outer_start_providing:
        HashMap<kad::QueryId, (proto::Hash, ResponseChannel<ObjectResponse>)>,
    pub pending_outer_delete_object:
        HashMap<OutboundRequestId, oneshot::Sender<Result<ResultObject>>>,

    pub pending_outbound_queries: HashMap<OutboundRequestId, oneshot::Sender<Result<TypedObject>>>,
}

impl BehaviourContext {
    pub fn new() -> Self {
        BehaviourContext {
            providing: HashMap::new(),
            pending_inner_start_providing: HashMap::new(),
            pending_outer_start_providing: HashMap::new(),
            pending_inner_send_object: HashMap::new(),
            pending_inner_get_providers: HashMap::new(),
            pending_inner_get_object: HashMap::new(),
            pending_inner_dial: HashMap::new(),
            pending_inner_get_closest_peers: HashMap::new(),
            pending_outer_delete_object: HashMap::new(),
            pending_outbound_queries: HashMap::new(),
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
            LiberumNetoBehaviorEvent::QuerySender(e) => {
                self.handle_query_sender(e).await;
            }
        }
    }
}
