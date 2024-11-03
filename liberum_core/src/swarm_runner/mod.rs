use crate::node::{self, BootstrapNode, Node};
use anyhow::Result;
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use kameo::actor::ActorRef;
use kameo::request::MessageSend;
use libp2p::{
    identity,
    kad::{self},
    Multiaddr, StreamProtocol, SwarmBuilder,
};
use libp2p::{
    kad::{store::MemoryStore, Behaviour},
    swarm::SwarmEvent,
    Swarm,
};
use std::str::FromStr;
use tracing::{debug, error, info, warn};

const IPFS_PROTO_NAME: StreamProtocol = StreamProtocol::new("/ipfs/kad/1.0.0");
const DEFAULT_MULTIADDR_STR: &str = "/ip6/::/udp/0/quic-v1"; // "/ipv/::/udp/0/quic-v1"

pub enum SwarmRunnerError {}

pub enum SwarmRunnerMessage {
    Echo {
        message: String,
        resp: oneshot::Sender<Result<String, SwarmRunnerError>>,
    },
}

struct SwarmContext {
    swarm: Swarm<Behaviour<MemoryStore>>,
    node: Node,
}

// #[derive(NetworkBehaviour)]
// pub struct LiberumNetoBehavior {
//     kademlia: kad::Behaviour<kad::store::MemoryStore>,
// }

pub async fn run_swarm(node_ref: ActorRef<Node>, receiver: mpsc::Receiver<SwarmRunnerMessage>) {
    if let Err(e) = run_swarm_inner(node_ref.clone(), receiver).await {
        error!(err = e.to_string(), "Swarm run error");
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
            Behaviour::with_config(id, store, conf)
        })
        .inspect_err(|e| error!(err = e.to_string(), "could not create swarm"))?
        .build();

    for node in &node_data.bootstrap_nodes {
        swarm
            .behaviour_mut()
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
    };

    loop {
        tokio::select! {
            Some(message) = receiver.next() => {
                handle_swarm_runner_message(message, &mut context)?;
            }
            event = context.swarm.select_next_some() => {
                handle_swarm_event(event, &mut context)?;
            }
        }
    }
}

fn handle_swarm_runner_message(
    message: SwarmRunnerMessage,
    _swarm: &mut SwarmContext,
) -> Result<()> {
    match message {
        SwarmRunnerMessage::Echo { message, resp } => {
            debug!(message = message, "Received Echo!");
            let _ = resp.send(Ok(message));
        }
    }

    Ok(())
}

fn handle_swarm_event(event: SwarmEvent<kad::Event>, context: &mut SwarmContext) -> Result<()> {
    match event {
        libp2p::swarm::SwarmEvent::IncomingConnection {
            connection_id: _,
            local_addr: _,
            send_back_addr,
        } => {
            warn!(
                node = context.node.name,
                "Connection from {send_back_addr:?}"
            );
        }
        libp2p::swarm::SwarmEvent::Dialing {
            peer_id,
            connection_id: _,
        } => {
            warn!(node = context.node.name, "Dialing {peer_id:?}");
        }
        libp2p::swarm::SwarmEvent::NewListenAddr {
            listener_id: _,
            address,
        } => {
            let node = BootstrapNode {
                id: context.swarm.local_peer_id().clone(),
                addr: address.clone(),
            };
            let node = serde_json::to_string(&node)?;
            info!(node = context.node.name, "Listening! <{node}>");
        }
        libp2p::swarm::SwarmEvent::Behaviour(_) => {}
        _ => debug!(
            node = context.node.name,
            event = format!("{event:?}"),
            "Received Swarm Event!"
        ),
    }

    Ok(())
}
