use crate::node::{self, BootstrapNode, Node};
use anyhow::anyhow;
use anyhow::Result;
use futures::StreamExt;
use kameo::actor::ActorRef;
use kameo::request::MessageSend;
use libp2p::multiaddr::Protocol;
use libp2p::request_response::OutboundRequestId;
use libp2p::request_response::ProtocolSupport;
use libp2p::swarm::NetworkBehaviour;
use libp2p::PeerId;
use libp2p::{identity, kad, Multiaddr, StreamProtocol, SwarmBuilder};
use libp2p::{kad::store::MemoryStore, request_response, swarm::SwarmEvent, Swarm};
use serde::{Deserialize, Serialize};
use std::collections::hash_map;
use std::collections::HashSet;
use std::time::Duration;
use std::{collections::HashMap, path::PathBuf, str::FromStr};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info};

const KAD_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/kad/1.0.0");
const FILE_SHARE_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/file-share/1.0.0");
const DEFAULT_MULTIADDR_STR: &str = "/ip6/::/udp/0/quic-v1";

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

enum SharedResource {
    File { path: PathBuf },
}
struct SwarmContext {
    swarm: Swarm<LiberumNetoBehavior>,
    node: Node,
    published: HashMap<kad::RecordKey, SharedResource>,
    pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<()>>,
    pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pending_download_file: HashMap<OutboundRequestId, oneshot::Sender<Vec<u8>>>,
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<()>>>,
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
    let swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_quic()
        .with_behaviour(|key| {
            let conf = kad::Config::new(KAD_PROTO_NAME);
            let store = MemoryStore::new(key.public().to_peer_id());
            let kademlia = kad::Behaviour::with_config(id, store, conf);
            let req_resp = request_response::cbor::Behaviour::<FileRequest, FileResponse>::new(
                [(FILE_SHARE_PROTO_NAME, ProtocolSupport::Full)],
                request_response::Config::default(),
            );
            LiberumNetoBehavior {
                kademlia,
                file_share: req_resp,
            }
        })
        .inspect_err(|e| error!(err = e.to_string(), "could not create behavior"))?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let mut context = SwarmContext {
        node: node_data,
        swarm: swarm,
        published: HashMap::new(),
        pending_start_providing: HashMap::new(),
        pending_get_providers: HashMap::new(),
        pending_download_file: HashMap::new(),
        pending_dial: HashMap::new(),
    };

    let swarm_default_addr = Multiaddr::from_str(DEFAULT_MULTIADDR_STR).inspect_err(|e| {
        error!(
            err = e.to_string(),
            addr = DEFAULT_MULTIADDR_STR,
            "Could not create swarm listen address"
        );
    })?;

    if context.node.external_addresses.is_empty() {
        context
            .swarm
            .add_external_address(swarm_default_addr.clone());
        context.swarm.listen_on(swarm_default_addr.clone())?;
    } else {
        for addr in &context.node.external_addresses {
            context.swarm.add_external_address(addr.clone());
            context.swarm.listen_on(addr.clone())?;
        }
    }

    context
        .swarm
        .behaviour_mut()
        .kademlia
        .set_mode(Some(kad::Mode::Server));

    debug!(node_name = context.node.name, "Starting a swarm!");

    for node in &context.node.bootstrap_nodes {
        context
            .swarm
            .behaviour_mut()
            .kademlia
            .add_address(&node.id, node.addr.clone());
        context
            .swarm
            .dial(node.addr.clone().with(Protocol::P2p(node.id)))?;
        debug!("Bootstrap node: {}", serde_json::to_string(&node)?);
    }
    context
        .swarm
        .behaviour_mut()
        .kademlia
        .set_mode(Some(kad::Mode::Server));
    //let _ = context.swarm.behaviour_mut().kademlia.bootstrap().inspect_err(|e| debug!(err=e.to_string(), "bootstrap err"));

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
            SwarmRunnerMessage::Dial {
                peer_id,
                peer_addr,
                sender,
            } => {
                if let hash_map::Entry::Vacant(e) = self.pending_dial.entry(peer_id) {
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
                if self.published.contains_key(&id) {
                    info!(
                        node = self.node.name,
                        id = format!("{id:?}"),
                        "File is already published"
                    );
                    return Ok(false);
                }
                self.published
                    .insert(id.clone(), SharedResource::File { path: path.clone() });
                let qid = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(id.clone())?;
                self.pending_start_providing.insert(qid, sender);
                Ok(false)
            }
            SwarmRunnerMessage::GetProviders { id, sender } => {
                let query_id = self.swarm.behaviour_mut().kademlia.get_providers(id);
                self.pending_get_providers.insert(query_id, sender);
                Ok(false)
            }
            SwarmRunnerMessage::DownloadFile { id, peer, sender } => {
                let qid = self
                    .swarm
                    .behaviour_mut()
                    .file_share
                    .send_request(&peer, FileRequest { id: id.to_vec() });
                self.pending_download_file.insert(qid, sender);
                Ok(false)
            }
        }
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<LiberumNetoBehaviorEvent>,
    ) -> Result<()> {
        match event {
            SwarmEvent::Behaviour(LiberumNetoBehaviorEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result: kad::QueryResult::StartProviding(_),
                    ..
                },
            )) => {
                info!(
                    node = self.node.name,
                    id = format!("{id:?}"),
                    "Published file"
                );
                let sender: oneshot::Sender<()> = self
                    .pending_start_providing
                    .remove(&id)
                    .expect("Query ID to not disappear from hashmap.");
                let _ = sender.send(());
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
                if let Some(sender) = self.pending_get_providers.remove(&id) {
                    sender.send(providers).expect("Channel not to break");
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .query_mut(&id)
                        .unwrap()
                        .finish();
                }
            }
            SwarmEvent::Behaviour(LiberumNetoBehaviorEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result:
                        kad::QueryResult::GetProviders(Ok(
                            kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. },
                        )),
                    ..
                },
            )) => {
                debug!("Get providers didn't find any new records");
                if let Some(sender) = self.pending_get_providers.remove(&id) {
                    sender.send(HashSet::new()).expect("Channel not to break");
                    //self.swarm.behaviour_mut().kademlia.query_mut(&id).unwrap().finish();
                }
            }
            SwarmEvent::Behaviour(LiberumNetoBehaviorEvent::Kademlia(
                kad::Event::InboundRequest {
                    request: kad::InboundRequest::GetProvider { .. },
                },
            )) => {
                debug!(node = self.node.name, "Received GetProvider")
            }
            SwarmEvent::Behaviour(LiberumNetoBehaviorEvent::Kademlia(_)) => {}
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                let addr = endpoint.get_remote_address().clone();
                info!(
                    peer_id = format!("{peer_id:?}"),
                    address = format!("{addr}"),
                    "New connection, adding peer address"
                );
                self.swarm
                    .add_peer_address(peer_id, endpoint.get_remote_address().clone());
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, addr);

                debug!(node = self.node.name, "Neighbours:");
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .kbuckets()
                    .for_each(|k| {
                        k.iter().for_each(|e| {
                            debug!("neighbour: {:?}: {:?}", e.node.key, e.node.value);
                        });
                    });
            }
            SwarmEvent::ConnectionClosed { .. } => {}
            SwarmEvent::Dialing {
                peer_id: Some(peer_id),
                ..
            } => {
                debug!("Dialing {peer_id}");
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
                        .pending_download_file
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
}
