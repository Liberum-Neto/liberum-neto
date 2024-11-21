pub mod file_share;
pub mod kademlia;
use anyhow::Result;
use std::collections::{HashMap, HashSet};

use file_share::*;
use libp2p::{
    kad,
    request_response::{self, OutboundRequestId},
    swarm::NetworkBehaviour,
    PeerId,
};
use tokio::sync::oneshot;

use super::SwarmContext;

///! The module contains the definition of the behaviour of the network

/// The behaviour of the network
#[derive(NetworkBehaviour)]
pub struct LiberumNetoBehavior {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub file_share: request_response::cbor::Behaviour<FileRequest, FileResponse>,
}

/// Data required to handle events from the behaviours. Mostly
/// hashmaps to store query IDs and senders for the requests
pub struct BehaviourContext {
    /// A hashmap of resources that are provided by the node. Should be replaced with
    /// an implementation of VAULT
    pub providing: HashMap<kad::RecordKey, SharedResource>,
    pub pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<Result<()>>>,
    pub pending_publish_file: HashMap<kad::QueryId, oneshot::Sender<Result<()>>>,
    pub pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pub pending_download_file: HashMap<OutboundRequestId, oneshot::Sender<Result<Vec<u8>>>>,
    pub pending_dial: HashMap<PeerId, oneshot::Sender<Result<()>>>,
}

impl BehaviourContext {
    pub fn new() -> Self {
        BehaviourContext {
            providing: HashMap::new(),
            pending_start_providing: HashMap::new(),
            pending_publish_file: HashMap::new(),
            pending_get_providers: HashMap::new(),
            pending_download_file: HashMap::new(),
            pending_dial: HashMap::new(),
        }
    }
}

impl SwarmContext {
    pub(crate) async fn handle_behaviour_event(&mut self, event: LiberumNetoBehaviorEvent) {
        match event {
            LiberumNetoBehaviorEvent::Kademlia(e) => {
                self.handle_kademlia(e);
            }
            LiberumNetoBehaviorEvent::FileShare(e) => {
                self.handle_file_share(e).await;
            }
        }
    }
}
