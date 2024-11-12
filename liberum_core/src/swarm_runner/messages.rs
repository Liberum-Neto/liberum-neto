use super::behaviour::file_share;
use super::SwarmContext;
use anyhow::anyhow;
use anyhow::Result;
use bincode::de;
use libp2p::multiaddr::Protocol;
use libp2p::PeerId;
use libp2p::{kad, Multiaddr};
use std::collections::hash_map;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::oneshot;
use tracing::{debug, error, info};
use tracing_subscriber::field::debug;

pub enum SwarmRunnerError {}

/// Messages that can be send from a Node to the SwarmRunner
pub enum SwarmRunnerMessage {
    Echo {
        message: String,
        response_sender: oneshot::Sender<Result<String, SwarmRunnerError>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        response_sender: oneshot::Sender<Result<()>>,
    },
    Kill,
    GetProviders {
        id: kad::RecordKey,
        response_sender: oneshot::Sender<HashSet<PeerId>>,
    },
    ProvideFile {
        id: kad::RecordKey,
        path: PathBuf,
        response_sender: oneshot::Sender<Result<()>>,
    },
    DownloadFileRequestResponse {
        id: kad::RecordKey,
        peer: PeerId,
        response_sender: oneshot::Sender<Vec<u8>>,
    },
    DownloadFileDHT {
        id: kad::RecordKey,
        response_sender: oneshot::Sender<Vec<u8>>,
    },
    PublishFile {
        record: kad::Record,
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
                if let hash_map::Entry::Vacant(entry) = self.behaviour.pending_dial.entry(peer_id) {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, peer_addr.clone());
                    match self.swarm.dial(peer_addr) {
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
            SwarmRunnerMessage::ProvideFile {
                id,
                path,
                response_sender,
            } => {
                if self.behaviour.providing.contains_key(&id) {
                    info!(
                        node = self.node.name,
                        id = format!("{id:?}"),
                        "File is already being provided"
                    );
                    return Ok(false);
                }
                // Add the file to the providing list
                self.behaviour.providing.insert(
                    id.clone(),
                    file_share::SharedResource::File(file_share::FileResource {
                        path: path.clone(),
                    }),
                );
                // Strat a query to be providing the file ID in kademlia
                let qid = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(id.clone())?;
                self.behaviour
                    .pending_start_providing
                    .insert(qid, response_sender);
                Ok(false)
            }

            // Get providers for a file ID
            SwarmRunnerMessage::GetProviders {
                id,
                response_sender,
            } => {
                let query_id = self.swarm.behaviour_mut().kademlia.get_providers(id);
                self.behaviour
                    .pending_get_providers
                    .insert(query_id, response_sender);
                Ok(false)
            }

            SwarmRunnerMessage::DownloadFileRequestResponse {
                id,
                peer,
                response_sender,
            } => {
                let qid = self
                    .swarm
                    .behaviour_mut()
                    .file_share
                    .send_request(&peer, file_share::FileRequest { id: id.to_vec() });

                self.behaviour
                    .pending_download_file
                    .insert(qid, response_sender);
                Ok(false)
            }

            SwarmRunnerMessage::DownloadFileDHT {
                id,
                response_sender,
            } => {
                debug!("Downloading file from DHT {:?}", id);
                let qid = self.swarm.behaviour_mut().kademlia.get_record(id);
                self.behaviour
                    .pending_download_file_dht
                    .insert(qid, response_sender);
                Ok(false)
            }

            SwarmRunnerMessage::PublishFile {
                record,
                response_sender,
            } => {
                debug!("Publishing file {:?}", record.key);
                let qid = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .put_record(record, kad::Quorum::One)?; // at least one node MUST store to consider it successful, counting itself

                self.behaviour
                    .pending_publish_file
                    .insert(qid, response_sender);
                Ok(false)
            }
        }
    }
}
