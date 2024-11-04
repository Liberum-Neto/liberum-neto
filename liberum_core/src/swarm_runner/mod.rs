use crate::node::{self, BootstrapNode, Node};
use anyhow::anyhow;
use anyhow::Result;
use futures::StreamExt;
use kameo::actor::ActorRef;
use kameo::request::MessageSend;
use libp2p::kad::QueryId;
use libp2p::kad::QueryResult;
use libp2p::request_response::OutboundRequestId;
use libp2p::request_response::ProtocolSupport;
use libp2p::swarm::NetworkBehaviour;
use libp2p::PeerId;
use libp2p::{
    identity,
    kad::{self, InboundRequest},
    Multiaddr, StreamProtocol, SwarmBuilder,
};
use libp2p::{
    kad::{store::MemoryStore, Behaviour},
    request_response,
    swarm::SwarmEvent,
    Swarm,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read;
use std::{collections::HashMap, path::PathBuf, str::FromStr};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

const IPFS_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/kad/1.0.0");
const FILE_SHARE_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/file-share/1.0.0");
const DEFAULT_MULTIADDR_STR: &str = "/ip6/::/udp/0/quic-v1"; // "/ipv/::/udp/0/quic-v1"

pub enum SwarmRunnerError {}

pub enum SwarmRunnerMessage {
    Echo {
        message: String,
        resp: oneshot::Sender<Result<String, SwarmRunnerError>>,
    },
    Kill,
    GetProviders {
        id: libp2p::kad::RecordKey,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    PublishFile {
        id: libp2p::kad::RecordKey,
        path: PathBuf,
    },
    // DownloadFile {
    //     id: libp2p::kad::RecordKey,
    //     sender: oneshot::Sender<HashSet<PeerId>>,
    // }
}

enum SharedResource {
    File { path: PathBuf },
}
struct SwarmContext {
    swarm: Swarm<LiberumNetoBehavior>,
    node: Node,
    published: HashMap<kad::RecordKey, SharedResource>,
    pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pending_get_file: HashMap<OutboundRequestId, oneshot::Sender<Vec<u8>>>,
}

#[derive(NetworkBehaviour)]
pub struct LiberumNetoBehavior {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    file_share: request_response::cbor::Behaviour<FileRequest, FileResponse>,
}
#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct FileRequest {
    id: Vec<u8>,
}
#[derive(Serialize, Deserialize, Debug, Hash, PartialEq)]
pub struct FileResponse {
    data: Vec<u8>,
}

pub async fn run_swarm(node_ref: ActorRef<Node>, receiver: mpsc::Receiver<SwarmRunnerMessage>) {
    if let Err(e) = run_swarm_inner(node_ref.clone(), receiver).await {
        error!(err = format!("{e:?}"), "Swarm run error");
        node_ref.ask(node::SwarmDied).send().await.unwrap();
    }
}

async fn run_swarm_inner(
    node_ref: ActorRef<Node>,
    mut receiver: mpsc::Receiver<SwarmRunnerMessage>,
) -> Result<()> {
    // It must be guaranteed not to ever fail. Swarm can't start without this data.
    // If it fails then it's a bug

    let node_data = node_ref
        .ask(node::GetSnapshot {})
        .send()
        .await
        .inspect_err(|e| error!(err = e.to_string(), "Swarm can't get node snapshot!"))?;

    let keypair = node_data.keypair.clone();
    let id = identity::PeerId::from_public_key(&keypair.public());
    let mut swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_quic()
        .with_behaviour(|key| {
            let conf = kad::Config::new(IPFS_PROTO_NAME);
            let store = MemoryStore::new(key.public().to_peer_id());
            let kademlia = Behaviour::with_config(id, store, conf);
            let req_resp = request_response::cbor::Behaviour::<FileRequest, FileResponse>::new(
                [(FILE_SHARE_PROTO_NAME, ProtocolSupport::Full)],
                request_response::Config::default(),
            );
            LiberumNetoBehavior {
                kademlia,
                file_share: req_resp,
            }
        })
        .inspect_err(|e| error!(err = e.to_string(), "could not create swarm"))?
        .build();

    for node in &node_data.bootstrap_nodes {
        swarm
            .behaviour_mut()
            .kademlia
            .add_address(&node.id, node.addr.clone());
        debug!("Bootstrap node: {}", serde_json::to_string(&node)?);
    }

    let swarm_default_addr = Multiaddr::from_str(DEFAULT_MULTIADDR_STR).inspect_err(|e| {
        error!(
            err = e.to_string(),
            addr = DEFAULT_MULTIADDR_STR,
            "Could not create swarm listen address"
        );
    })?;

    if node_data.external_addresses.is_empty() {
        swarm.add_external_address(swarm_default_addr.clone());
        swarm.listen_on(swarm_default_addr.clone())?;
    } else {
        for addr in &node_data.external_addresses {
            swarm.add_external_address(addr.clone());
            swarm.listen_on(addr.clone())?;
        }
    }

    debug!(node_name = node_data.name, "Starting a swarm!");

    let mut context = SwarmContext {
        node: node_data,
        swarm: swarm,
        published: HashMap::new(),
        pending_get_providers: HashMap::new(),
        pending_get_file: HashMap::new(),
    };

    loop {
        tokio::select! {
            Some(message) = receiver.recv() => {
                let should_end = context.handle_swarm_runner_message(message).await?;

                if should_end {
                    return Ok(());
                }
            }
            event = context.swarm.select_next_some() => {
                context.handle_swarm_event(event).await?;
            }
        }
    }
}

impl SwarmContext {
    async fn handle_swarm_runner_message(&mut self, message: SwarmRunnerMessage) -> Result<bool> {
        match message {
            SwarmRunnerMessage::Echo { message, resp } => {
                debug!(message = message, "Received Echo!");
                let _ = resp.send(Ok(message));
                Ok(false)
            }
            SwarmRunnerMessage::Kill => Ok(true),
            SwarmRunnerMessage::PublishFile { id, path } => {
                if self.published.contains_key(&id) {
                    return Err(anyhow!("asd"));
                }
                self.published
                    .insert(id.clone(), SharedResource::File { path: path.clone() });
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(id.clone())?;
                let id = liberum_core::file_id_to_str(id).await;
                debug!(path = format!("{path:?}"), id = id, "Providing file!");
                Ok(false)
            }
            SwarmRunnerMessage::GetProviders { id, sender } => {
                let query_id = self.swarm.behaviour_mut().kademlia.get_providers(id);
                self.pending_get_providers.insert(query_id, sender);
                Ok(false)
            }
        }
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<LiberumNetoBehaviorEvent>,
    ) -> Result<()> {
        match event {
            SwarmEvent::IncomingConnection {
                connection_id: _,
                local_addr: _,
                send_back_addr,
            } => {
                warn!(node = self.node.name, "Connection from {send_back_addr:?}");
            }
            SwarmEvent::Dialing {
                peer_id,
                connection_id: _,
            } => {
                warn!(node = self.node.name, "Dialing {peer_id:?}");
            }
            SwarmEvent::NewListenAddr {
                listener_id: _,
                address,
            } => {
                let node = BootstrapNode {
                    id: self.swarm.local_peer_id().clone(),
                    addr: address.clone(),
                };
                let node = serde_json::to_string(&node)?;
                info!(node = self.node.name, "Listening! <{node}>");
            }
            SwarmEvent::Behaviour(LiberumNetoBehaviorEvent::Kademlia(
                kad::Event::InboundRequest { request },
            )) => {
                self.handle_kad_request(request)?;
            }
            SwarmEvent::Behaviour(LiberumNetoBehaviorEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result:
                        kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                            providers,
                            ..
                        })),
                    ..
                },
            )) => {
                let sender = self.pending_get_providers.remove(&id);
                match sender {
                    None => {
                        error!("Get providers response sender should not have disappeared");
                        return Err(anyhow!("GetProviders response Sender disappeared"));
                    }
                    Some(sender) => {
                        let _ = sender
                            .send(providers)
                            .inspect_err(|e| error!("Failed to send get providers response"));
                    } // TODO unhandled
                }
            }
            SwarmEvent::Behaviour(LiberumNetoBehaviorEvent::FileShare(
                request_response::Event::Message { message, .. },
            )) => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    debug!("Request_response request!");
                    let id = kad::RecordKey::from(request.id.clone());
                    let file = self.published.get(&id);
                    if let Some(file) = file {
                        match file {
                            SharedResource::File { path } => {
                                let data = tokio::fs::read(path).await?;
                                self.swarm
                                    .behaviour_mut()
                                    .file_share
                                    .send_response(channel, FileResponse { data })
                                    .expect("Connection to peer to be still open.");
                            }
                        }
                    }
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    debug!("Request_response response!");
                    let _ = self
                        .pending_get_file
                        .remove(&request_id)
                        .expect("Request to still be pending.")
                        .send(response.data);
                }
            },

            _ => debug!(
                node = self.node.name,
                event = format!("{event:?}"),
                "Received Swarm Event!"
            ),
        }

        Ok(())
    }

    fn handle_kad_request(&mut self, request: InboundRequest) -> Result<()> {
        match request {
            InboundRequest::FindNode { num_closer_peers } => {
                debug!(
                    num_closer_peers = num_closer_peers,
                    node = self.node.name,
                    "kad: FindNode"
                )
            }
            InboundRequest::GetProvider {
                num_closer_peers,
                num_provider_peers,
            } => {
                debug!(
                    num_closer_peers = num_closer_peers,
                    num_provider_peers = num_provider_peers,
                    node = self.node.name,
                    "kad: GetProvider"
                )
            }
            InboundRequest::AddProvider { record } => {
                debug!(
                    record = format!("{record:?}"),
                    node = self.node.name,
                    "kad: AddProvider"
                )
            }
            InboundRequest::GetRecord {
                num_closer_peers,
                present_locally,
            } => {
                debug!(
                    num_closer_peers = num_closer_peers,
                    present_locally = present_locally,
                    node = self.node.name,
                    "kad: GetRecord"
                )
            }
            InboundRequest::PutRecord {
                source,
                connection,
                record,
            } => {
                debug!(
                    source = format!("{source:?}"),
                    connection = format!("{connection:?}"),
                    record = format!("{record:?}"),
                    node = self.node.name,
                    "kad: PutRecord"
                )
            }
        }
        Ok(())
    }
}
