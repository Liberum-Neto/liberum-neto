use crate::connection::ConnectionContext;
use crate::node::manager::{CreateNodes, StartNodes};
use crate::node::Node;
use kameo::request::MessageSend;
use liberum_core::messages::*;
use libp2p::identity::Keypair;
use tracing::debug;

pub async fn handle_new_nodes(names: Vec<String>, context: &mut ConnectionContext) -> DaemonResult {
    let mut nodes = Vec::with_capacity(names.len());
    for name in names {
        let node = Node::builder()
            .name(name)
            .keypair(Keypair::generate_ed25519())
            .build();

        match node {
            Err(e) => {
                return Err(DaemonError::Other(e.to_string()));
            }
            Ok(node) => {
                nodes.push(node);
            }
        }
    }

    let resp = context.node_manager.ask(CreateNodes { nodes }).send().await;
    match resp {
        Err(e) => Err(DaemonError::Other(e.to_string())),
        Ok(_resp) => Ok(DaemonResponse::NodeCreated),
    }
}

pub async fn handle_start_nodes(
    names: Vec<String>,
    context: &mut ConnectionContext,
) -> DaemonResult {
    let resp = context.node_manager.ask(StartNodes { names }).send().await;
    match resp {
        Err(e) => Err(DaemonError::Other(e.to_string())),
        Ok(nodes) => {
            for _node in nodes {
                //debug!(node = node.name, "Node loaded!");
            }
            Ok(DaemonResponse::NodeStarted)
        }
    }
}

pub async fn handle_stop_nodes(
    names: Vec<String>,
    _context: &mut ConnectionContext,
) -> DaemonResult {
    for name in names {
        debug!(node = name, "Node stopped!");
    }
    Ok(DaemonResponse::NodeStopped)
}

pub async fn handle_list_nodes(_context: &mut ConnectionContext) -> DaemonResult {
    Ok(DaemonResponse::NodeList(vec![]))
}
