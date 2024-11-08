use super::behaviour::file_share;
use super::SwarmContext;
use anyhow::anyhow;
use anyhow::Result;
use libp2p::multiaddr::Protocol;
use libp2p::PeerId;
use libp2p::{kad, Multiaddr};
use std::collections::hash_map;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::oneshot;
use tracing::{debug, info};

pub enum SwarmRunnerError {}

pub enum SwarmRunnerMessage {
    Echo {
        message: String,
        resp: oneshot::Sender<Result<String, SwarmRunnerError>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<()>>,
    },
    Kill,
    GetProviders {
        id: kad::RecordKey,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    PublishFile {
        id: kad::RecordKey,
        path: PathBuf,
        sender: oneshot::Sender<()>,
    },
    DownloadFile {
        id: kad::RecordKey,
        peer: PeerId,
        sender: oneshot::Sender<Vec<u8>>,
    },
}

impl SwarmContext {
    pub(crate) async fn handle_swarm_runner_message(
        &mut self,
        message: SwarmRunnerMessage,
    ) -> Result<bool> {
        match message {
            SwarmRunnerMessage::Echo { message, resp } => {
                debug!(message = message, "Received Echo!");
                let _ = resp.send(Ok(message));
                Ok(false)
            }
            SwarmRunnerMessage::Dial {
                peer_id,
                peer_addr,
                sender,
            } => {
                if let hash_map::Entry::Vacant(e) = self.behaviour.pending_dial.entry(peer_id) {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, peer_addr.clone());
                    match self.swarm.dial(peer_addr.with(Protocol::P2p(peer_id))) {
                        Ok(()) => {
                            e.insert(sender);
                        }
                        Err(e) => {
                            let _ = sender.send(Err(anyhow!(e)));
                        }
                    }
                } else {
                    debug!("Already dialing {peer_id}")
                }
                Ok(false)
            }
            SwarmRunnerMessage::Kill => Ok(true),
            SwarmRunnerMessage::PublishFile { id, path, sender } => {
                if self.behaviour.published.contains_key(&id) {
                    info!(
                        node = self.node.name,
                        id = format!("{id:?}"),
                        "File is already published"
                    );
                    return Ok(false);
                }
                self.behaviour.published.insert(
                    id.clone(),
                    file_share::SharedResource::File { path: path.clone() },
                );
                let qid = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(id.clone())?;
                self.behaviour.pending_start_providing.insert(qid, sender);
                Ok(false)
            }
            SwarmRunnerMessage::GetProviders { id, sender } => {
                let query_id = self.swarm.behaviour_mut().kademlia.get_providers(id);
                self.behaviour
                    .pending_get_providers
                    .insert(query_id, sender);
                Ok(false)
            }
            SwarmRunnerMessage::DownloadFile { id, peer, sender } => {
                let qid = self
                    .swarm
                    .behaviour_mut()
                    .file_share
                    .send_request(&peer, file_share::FileRequest { id: id.to_vec() });
                self.behaviour.pending_download_file.insert(qid, sender);
                Ok(false)
            }
        }
    }
}
