pub mod behaviour;
pub mod messages;

use crate::node::NodeSnapshot;
use crate::node::{self, Node};
use anyhow::anyhow;
use anyhow::Result;
use behaviour::*;
use futures::StreamExt;
use kameo::actor::ActorRef;
use kameo::request::MessageSend;
use liberum_core::node_config::BootstrapNode;
use libp2p::request_response::ProtocolSupport;
use libp2p::{identity, kad, Multiaddr, StreamProtocol, SwarmBuilder};
use libp2p::{kad::store::MemoryStore, request_response, swarm::SwarmEvent, Swarm};
use messages::*;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::warn;
use tracing::{debug, error, info};
const KAD_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/kad/1.0.0");
const FILE_SHARE_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/file-share/1.0.0");
const DEFAULT_MULTIADDR_STR_IP6: &str = "/ip6/::/udp/0/quic-v1";
const DEFAULT_MULTIADDR_STR_IP4: &str = "/ip4/0.0.0.0/udp/0/quic-v1";

///! Swarm Runner
///! This module is responsible for running the libp2p swarm and providing an interface
///! to the inner workings of the network to the Node actor.
///!
///! One swarm is run per Node actor. The actor communicates with the swarm using
///! an actor model of sending a message with a oneshot return channel via a mpsc channel.

/// The context of the swarm which holds all the data required to handle swarm events
/// and messages to the swarm runner
struct SwarmContext {
    swarm: Swarm<LiberumNetoBehavior>,
    node_snapshot: NodeSnapshot,
    behaviour: BehaviourContext,
}

/// Prepares the sender to send messages to the swarm
pub async fn run_swarm(node_ref: ActorRef<Node>) -> mpsc::Sender<SwarmRunnerMessage> {
    let (sender, receiver) = mpsc::channel::<SwarmRunnerMessage>(16);
    tokio::spawn(run_swarm_task(node_ref, receiver));
    sender
}

/// Task that runs the swarm and handles errors which can't be propagated outside of a task
async fn run_swarm_task(node_ref: ActorRef<Node>, receiver: mpsc::Receiver<SwarmRunnerMessage>) {
    if let Err(e) = run_swarm_main(node_ref.clone(), receiver).await {
        error!(err = format!("{e:?}"), "Swarm run error");
        node_ref.ask(node::SwarmDied).send().await.unwrap();
    }
}

/// The main function that runs the swarm
async fn run_swarm_main(
    node_ref: ActorRef<Node>,
    mut receiver: mpsc::Receiver<SwarmRunnerMessage>,
) -> Result<()> {
    // It must be guaranteed not to ever fail. Swarm can't start without this data.
    // If it fails then it's a bug

    // Get the node data
    let node_snapshot = node_ref
        .ask(node::GetSnapshot {})
        .send()
        .await
        .inspect_err(|e| error!(err = e.to_string(), "Swarm can't get node snapshot!"))?;

    // Create a new swarm using the node data
    let keypair = node_snapshot.keypair.clone();
    let id = identity::PeerId::from_public_key(&keypair.public());
    let swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_quic()
        .with_behaviour(|key| {
            let store_conf = kad::store::MemoryStoreConfig::default();
            let store = MemoryStore::with_config(key.public().to_peer_id(), store_conf);

            let mut conf = kad::Config::new(KAD_PROTO_NAME);

            conf.set_record_filtering(kad::StoreInserts::FilterBoth);
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
        node_snapshot,
        swarm: swarm,
        behaviour: BehaviourContext::new(),
    };

    let swarm_default_addr_ip6 =
        Multiaddr::from_str(DEFAULT_MULTIADDR_STR_IP6).inspect_err(|e| {
            error!(
                err = e.to_string(),
                addr = DEFAULT_MULTIADDR_STR_IP6,
                "Could not create swarm listen address IP6"
            );
        })?;
    let swarm_default_addr_ip4 =
        Multiaddr::from_str(DEFAULT_MULTIADDR_STR_IP4).inspect_err(|e| {
            error!(
                err = e.to_string(),
                addr = DEFAULT_MULTIADDR_STR_IP4,
                "Could not create swarm listen address IP4"
            );
        })?;

    let default_addr = vec![swarm_default_addr_ip6, swarm_default_addr_ip4];

    // Add the external addresses to the swarm
    if context.node_snapshot.external_addresses.is_empty() {
        for addr in default_addr {
            context.swarm.add_external_address(addr.clone());
            context.swarm.listen_on(addr.clone())?;
        }
    } else {
        for addr in &context.node_snapshot.external_addresses {
            context.swarm.add_external_address(addr.clone());
            context.swarm.listen_on(addr.clone())?;
        }
    }

    // Set mode to Server EXTREMELY IMPORTANT, otherwise the node won't
    // talk to anyone
    context
        .swarm
        .behaviour_mut()
        .kademlia
        .set_mode(Some(kad::Mode::Server));

    debug!(node_name = context.node_snapshot.name, "Starting a swarm!");

    // Bootstrap using the bootstrap nodes from the node data
    for node in &context.node_snapshot.bootstrap_nodes {
        context
            .swarm
            .behaviour_mut()
            .kademlia
            .add_address(&node.id, node.addr.clone());
        debug!("Bootstrap node: {}", serde_json::to_string(&node)?);
    }
    context
        .swarm
        .behaviour_mut()
        .kademlia
        .bootstrap()
        .inspect_err(|e| {
            info!(err = e.to_string(), "Could not bootstrap the swarm");
        })
        .ok();

    loop {
        tokio::select! {
            Some(message) = receiver.recv() => {
                let should_end = context.handle_swarm_runner_message(message).await?;

                // If the message returns true, then the swarm should end
                // Mainly used for the Kill message but might be useful in other cases
                if should_end {
                    return Ok(());
                }
            }
            event = context.swarm.select_next_some() => {
                context.handle_swarm_event(event).await?;
            }
            else => {break Err(anyhow!("Channel to Node closed"));}
        }
    }
}

/// Methods on SwarmContext for handling Swarm Events
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
                peer_id,
                endpoint,
                connection_id,
                ..
            } => {
                // If it was caused by using the Dial message, then send the response
                if endpoint.is_dialer() {
                    if let Some(sender) = self.behaviour.pending_dial.remove(&connection_id) {
                        let _ = sender.send(Ok(()));
                    }
                }

                let addr = endpoint.get_remote_address().clone();
                info!(
                    peer_id = format!("{peer_id:?}"),
                    address = format!("{addr}"),
                    "New connection"
                );
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, addr);
                self.print_neighbours();
            }
            SwarmEvent::OutgoingConnectionError {
                connection_id,
                peer_id,
                error,
            } => {
                warn!(
                    node = self.node.name,
                    peer_id = format!("{peer_id:?}"),
                    error = format!("{error}"),
                    "Outgoing connection error"
                );
                if let Some(sender) = self.behaviour.pending_dial.remove(&connection_id) {
                    let _ = sender.send(Err(anyhow!(error)));
                }
            }
            SwarmEvent::ConnectionClosed { .. } => {}
            SwarmEvent::NewListenAddr {
                listener_id: _,
                address,
            } => {
                let node = BootstrapNode {
                    id: self.swarm.local_peer_id().clone(),
                    addr: address.clone(),
                };
                let node = serde_json::to_string(&node)?;
                info!(node = self.node_snapshot.name, "Listening! <{node}>");
            }
            _ => debug!(
                node = self.node_snapshot.name,
                event = format!("{event:?}"),
                "Received Swarm Event!"
            ),
        }

        Ok(())
    }
}

/// Utility not related to behaviours
impl SwarmContext {
    fn print_neighbours(&mut self) {
        debug!(node = self.node_snapshot.name, "Neighbours:");
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
}
