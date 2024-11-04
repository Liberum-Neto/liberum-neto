use crate::node::{self, BootstrapNode, Node};
use anyhow::anyhow;
use anyhow::Result;
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use kameo::actor::ActorRef;
use kameo::request::MessageSend;
use libp2p::{
    identity,
    kad::{self, InboundRequest},
    Multiaddr, StreamProtocol, SwarmBuilder,
};
use libp2p::{
    kad::{store::MemoryStore, Behaviour},
    swarm::SwarmEvent,
    Swarm,
};
use std::{collections::HashMap, path::PathBuf, str::FromStr};
use tracing::{debug, error, info, warn};

const IPFS_PROTO_NAME: StreamProtocol = StreamProtocol::new("/ipfs/kad/1.0.0");
const DEFAULT_MULTIADDR_STR: &str = "/ip6/::/udp/0/quic-v1"; // "/ipv/::/udp/0/quic-v1"

pub enum SwarmRunnerError {}

pub enum SwarmRunnerMessage {
    Echo {
        message: String,
        resp: oneshot::Sender<Result<String, SwarmRunnerError>>,
    },
    Kill,
    PublishFile {
        id: libp2p::kad::RecordKey,
        path: PathBuf,
    },
}

enum SharedResource {
    File { path: PathBuf },
}
struct SwarmContext {
    swarm: Swarm<Behaviour<MemoryStore>>,
    node: Node,
    published: HashMap<kad::RecordKey, SharedResource>,
}

// #[derive(NetworkBehaviour)]
// pub struct LiberumNetoBehavior {
//     kademlia: kad::Behaviour<kad::store::MemoryStore>,
// }

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
        published: HashMap::new(),
    };

    loop {
        tokio::select! {
            Some(message) = receiver.next() => {
                let should_end = handle_swarm_runner_message(message, &mut context).await?;

                if should_end {
                    return Ok(());
                }
            }
            event = context.swarm.select_next_some() => {
                handle_swarm_event(event, &mut context)?;
            }
        }
    }
}

async fn handle_swarm_runner_message(
    message: SwarmRunnerMessage,
    swarm: &mut SwarmContext,
) -> Result<bool> {
    match message {
        SwarmRunnerMessage::Echo { message, resp } => {
            debug!(message = message, "Received Echo!");
            let _ = resp.send(Ok(message));
            Ok(false)
        }
        SwarmRunnerMessage::Kill => Ok(true),
        SwarmRunnerMessage::PublishFile { id, path } => {
            if swarm.published.contains_key(&id) {
                return Err(anyhow!("asd"));
            }
            swarm
                .published
                .insert(id.clone(), SharedResource::File { path: path.clone() });
            swarm.swarm.behaviour_mut().start_providing(id.clone())?;
            let id = liberum_core::file_id_to_str(id).await;
            debug!(path = format!("{path:?}"), id = id, "Providing file!");
            Ok(false)
        }
    }
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
        libp2p::swarm::SwarmEvent::Behaviour(kad::Event::InboundRequest { request }) => {
            handle_kad_request(request, context)?;
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

fn handle_kad_request(request: InboundRequest, context: &mut SwarmContext) -> Result<()> {
    match request {
        InboundRequest::FindNode { num_closer_peers } => {
            debug!(
                num_closer_peers = num_closer_peers,
                node = context.node.name,
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
                node = context.node.name,
                "kad: GetProvider"
            )
        }
        InboundRequest::AddProvider { record } => {
            debug!(
                record = format!("{record:?}"),
                node = context.node.name,
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
                node = context.node.name,
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
                node = context.node.name,
                "kad: PutRecord"
            )
        }
    }
    Ok(())
}
