use crate::node::{self, Node};
use anyhow::Result;
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use kameo::actor::ActorRef;
use kameo::request::MessageSend;
use libp2p::{identity, kad, Multiaddr, StreamProtocol, SwarmBuilder};
use libp2p::{
    kad::{store::MemoryStore, Behaviour},
    swarm::SwarmEvent,
    Swarm,
};
use std::str::FromStr;
use tracing::{debug, error};

const IPFS_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/kad/1.0.0");
const DEFAULT_MULTIADDR_STR: &str = "/ip6/::/quic-v1/0";

pub enum SwarmRunnerError {}

pub enum SwarmRunnerMessage {
    Echo {
        message: String,
        resp: oneshot::Sender<Result<String, SwarmRunnerError>>,
    },
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
    }

    let swarm_default_addr = Multiaddr::from_str(DEFAULT_MULTIADDR_STR).inspect_err(|e| {
        error!(
            err = e.to_string(),
            addr = DEFAULT_MULTIADDR_STR,
            "Could not create swarm listen address"
        );
    })?;

    debug!("default addr: {}", swarm_default_addr);

    if node_data.external_addresses.is_empty() {
        swarm.add_external_address(swarm_default_addr.clone());
        swarm.listen_on(swarm_default_addr)?;
    } else {
        for addr in node_data.external_addresses {
            swarm.add_external_address(addr.clone());
            swarm.listen_on(addr)?;
        }
    }

    debug!(node_name = node_data.name, "Starting a swarm!");

    loop {
        tokio::select! {
            Some(message) = receiver.next() => {
                handle_swarm_runner_message(message, &mut swarm)?;
            }
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &mut swarm)?;
            }
        }
    }
}

fn handle_swarm_runner_message(
    message: SwarmRunnerMessage,
    _swarm: &mut Swarm<Behaviour<MemoryStore>>,
) -> Result<()> {
    match message {
        SwarmRunnerMessage::Echo { message, resp } => {
            debug!(message = message, "Received Echo!");
            let _ = resp.send(Ok(message));
        }
    }

    Ok(())
}

fn handle_swarm_event(
    event: SwarmEvent<kad::Event>,
    _swarm: &mut Swarm<Behaviour<MemoryStore>>,
) -> Result<()> {
    match event {
        _ => debug!(event = format!("{event:?}"), "Received Swarm Event!"),
    }

    Ok(())
}
