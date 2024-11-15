pub mod behaviour;
pub mod messages;

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
use tracing::{debug, error, info};
const KAD_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/kad/1.0.0");
const FILE_SHARE_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/file-share/1.0.0");
const DEFAULT_MULTIADDR_STR_IP6: &str = "/ip6/::/udp/0/quic-v1";
const DEFAULT_MULTIADDR_STR_IP4: &str = "/ip4/0.0.0.0/udp/0/quic-v1";

struct SwarmContext {
    swarm: Swarm<LiberumNetoBehavior>,
    node: Node,
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
    let node_data = node_ref
        .ask(node::GetSnapshot {})
        .send()
        .await
        .inspect_err(|e| error!(err = e.to_string(), "Swarm can't get node snapshot!"))?;

    // Create a new swarm using the node data
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
    if context.node.external_addresses.is_empty() {
        for addr in default_addr {
            context.swarm.add_external_address(addr.clone());
            context.swarm.listen_on(addr.clone())?;
        }
    } else {
        for addr in &context.node.external_addresses {
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

    debug!(node_name = context.node.name, "Starting a swarm!");

    // Bootstrap using the bootstrap nodes from the node data
    for node in &context.node.bootstrap_nodes {
        context
            .swarm
            .behaviour_mut()
            .kademlia
            .add_address(&node.id, node.addr.clone());
        debug!("Bootstrap node: {}", serde_json::to_string(&node)?);
    }

    // Main swarm loop for handling all events and messages
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
                peer_id, endpoint, ..
            } => {
                // If it was caused by using the Dial message, then send the response
                if endpoint.is_dialer() {
                    if let Some(sender) = self.behaviour.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Ok(()));
                    }
                }

                let addr = endpoint.get_remote_address().clone();
                info!(
                    peer_id = format!("{peer_id:?}"),
                    address = format!("{addr}"),
                    "New connection"
                );

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
