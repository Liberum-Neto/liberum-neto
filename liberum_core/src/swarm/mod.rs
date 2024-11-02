use kameo::{request::MessageSend, Actor};
use libp2p::*;
use swarm::NetworkBehaviour;
use kameo::actor::ActorRef;
use crate::node::{self, GetSnapshot, Node};
use tracing::{error,debug};


const IPFS_PROTO_NAME: StreamProtocol = StreamProtocol::new("/liberum/kad/1.0.0");
const DEFAULT_MULTIADDR_STR: &str = "/ip6/::/quic-v1/0";


#[derive(NetworkBehaviour)]
pub struct LiberumNetoBehavior {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

pub struct LiberumSwarm {
    swarm: LiberumNetoBehavior,
}

pub async fn start_swarm(node_ref: ActorRef<Node>) {
    // It must be guaranteed not to ever fail. Swarm can't start without this data.
    let node_data = node_ref.ask(node::GetSnapshot{}).send().await
    .inspect_err(|e|error!(err=e.to_string(), "Swarm can't get node snapshot!")).unwrap();

    let keypair = node_data.keypair.clone();
    let id = identity::PeerId::from_public_key(&keypair.public());
    let swarm = SwarmBuilder::with_existing_identity(keypair.clone())
            .with_tokio()
            .with_quic()
            .with_behaviour(|_key| {
                libp2p::kad::Behaviour::new(id, libp2p::kad::store::MemoryStore::new(id))
            }).unwrap() // TODO handle error
            .build();
        
    debug!(node_name=node_data.name, "Starting a swarm!");

    loop {
        // placeholder
    }
}


// for node in &self.bootstrap_nodes {
//     self.swarm
//         .behaviour_mut()
//         .add_address(&node.id, node.addr.clone());
// }

// // for addr in &self.addresses {
// //     self.swarm.add_external_address(addr.clone());
// // }
// // if self.addresses.is_empty() {
// //     self.swarm.add_external_address(Multiaddr::from_str(DEFAULT_MULTIADDR_STR)?);
// // }
// self.swarm
//     .listen_on(Multiaddr::from_str(DEFAULT_MULTIADDR_STR)?);
// Ok(())