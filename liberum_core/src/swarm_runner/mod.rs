pub mod behaviour;
pub mod messages;

use crate::node::{self, BootstrapNode, Node};
use anyhow::Result;
use behaviour::*;
use futures::StreamExt;
use kameo::actor::ActorRef;
use kameo::request::MessageSend;
use libp2p::multiaddr::Protocol;
use libp2p::request_response::ProtocolSupport;
use libp2p::{identity, kad, Multiaddr, StreamProtocol, SwarmBuilder};
use libp2p::{kad::store::MemoryStore, request_response, swarm::SwarmEvent, Swarm};
use messages::*;
use std::time::Duration;
use std::{collections::HashMap, str::FromStr};
use tokio::sync::mpsc;
use tracing::{debug, error, info};
const KAD_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/kad/1.0.0");
const FILE_SHARE_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/file-share/1.0.0");
const DEFAULT_MULTIADDR_STR: &str = "/ip6/::/udp/0/quic-v1";

struct SwarmContext {
    swarm: Swarm<LiberumNetoBehavior>,
    node: Node,
    behaviour: BehaviourContext,
}

pub async fn run_swarm(node_ref: ActorRef<Node>) -> mpsc::Sender<SwarmRunnerMessage> {
    let (sender, receiver) = mpsc::channel::<SwarmRunnerMessage>(16);
    tokio::spawn(run_swarm_task(node_ref, receiver));
    sender
}

async fn run_swarm_task(node_ref: ActorRef<Node>, receiver: mpsc::Receiver<SwarmRunnerMessage>) {
    if let Err(e) = run_swarm_main(node_ref.clone(), receiver).await {
        error!(err = format!("{e:?}"), "Swarm run error");
        node_ref.ask(node::SwarmDied).send().await.unwrap();
    }
}

async fn run_swarm_main(
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
            let req_resp = request_response::cbor::Behaviour::<
                file_share::FileRequest,
                file_share::FileResponse,
            >::new(
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
        behaviour: BehaviourContext {
            published: HashMap::new(),
            pending_start_providing: HashMap::new(),
            pending_get_providers: HashMap::new(),
            pending_download_file: HashMap::new(),
            pending_dial: HashMap::new(),
        },
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
    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<LiberumNetoBehaviorEvent>,
    ) -> Result<()> {
        match event {
            SwarmEvent::Behaviour(e) => {
                self.handle_behaviour_event(e).await;
            }
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
            _ => debug!(
                node = self.node.name,
                event = format!("{event:?}"),
                "Received Swarm Event!"
            ),
        }

        Ok(())
    }
}
