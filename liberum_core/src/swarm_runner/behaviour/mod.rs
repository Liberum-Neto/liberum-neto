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

#[derive(NetworkBehaviour)]
pub struct LiberumNetoBehavior {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub file_share: request_response::cbor::Behaviour<FileRequest, FileResponse>,
}
pub struct BehaviourContext {
    pub published: HashMap<kad::RecordKey, SharedResource>,
    pub pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<()>>,
    pub pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pub pending_download_file: HashMap<OutboundRequestId, oneshot::Sender<Vec<u8>>>,
    pub pending_dial: HashMap<PeerId, oneshot::Sender<Result<()>>>,
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
