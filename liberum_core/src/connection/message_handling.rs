use kameo::request::MessageSend;
use liberum_core::messages::*;
use crate::node::{self, Node};
use libp2p::identity::Keypair;
use crate::connection::ConnectionContext;


pub async fn handle_new_nodes(names: Vec<String>, context: &mut ConnectionContext) -> DaemonResult {
    //match config_manager.add_config(&name) {
    let mut nodes = Vec::with_capacity(names.len());
    for name in names {
        let node = Node::builder()
        .name(name)
        .keypair(Keypair::generate_ed25519())
        .build();
        if node.is_err() {
            return Err(DaemonError::Node(NodeError::Other(node.unwrap_err().to_string())));
        }
        nodes.push(node.unwrap());
    }

    let resp = context.node_store.ask(node::StoreNodes{0: nodes}).send().await;
    if resp.is_err() {
        return Err(DaemonError::Node(NodeError::Other(resp.unwrap_err().to_string())));
    }
    Ok(DaemonResponse::NodeResponse(NodeResponse::Created))
}


pub async fn handle_start_nodes(names: Vec<String>, context: &mut ConnectionContext) -> DaemonResult {
    let resp = context.node_store.ask(node::LoadNodes{0:names}).send().await;
    if resp.is_err() {
        return Err(DaemonError::Node(NodeError::Other(resp.unwrap_err().to_string())));
    }
    let nodes = resp.unwrap();
    for node in nodes {
        println!("Node {} laoded!", node.name);
    }
    Ok(DaemonResponse::NodeResponse(NodeResponse::Started))
}


pub async fn handle_stop_nodes(_context: &mut ConnectionContext) -> DaemonResult {
    Ok(DaemonResponse::NodeResponse(NodeResponse::Stopped))
}


pub async fn handle_list_nodes(_context: &mut ConnectionContext) -> DaemonResult {
    Ok(DaemonResponse::NodeResponse(NodeResponse::List(vec![])))
}
