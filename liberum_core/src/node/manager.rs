use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use kameo::{
    actor::ActorRef, mailbox::bounded::BoundedMailbox, message::Message, request::MessageSend,
    Actor,
};

use crate::node::store::LoadNodes;

use super::{store::NodeStore, Node};

#[derive(Debug)]
pub struct NodeManager {
    nodes: HashMap<String, ActorRef<Node>>,
    store: ActorRef<NodeStore>,
    actor_ref: Option<ActorRef<NodeManager>>,
}

impl NodeManager {
    pub fn new(store: ActorRef<NodeStore>) -> Self {
        NodeManager {
            nodes: HashMap::new(),
            store,
            actor_ref: None,
        }
    }
}

impl Actor for NodeManager {
    type Mailbox = BoundedMailbox<Self>;

    async fn on_start(
        &mut self,
        actor_ref: ActorRef<Self>,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        self.actor_ref = Some(actor_ref);
        Ok(())
    }
}

struct StartNode {
    name: String,
}

struct StopNode {
    name: String,
}

struct GetNode {
    name: String,
}

impl Message<StartNode> for NodeManager {
    type Reply = Result<ActorRef<Node>>;

    async fn handle(
        &mut self,
        StartNode { name }: StartNode,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        if self.nodes.contains_key(&name) {
            bail!("node is already started");
        }

        let mut node = self
            .store
            .ask(LoadNodes {
                names: vec![name.clone()],
            })
            .send()
            .await?
            .swap_remove(0);
        node.manager_ref = self.actor_ref.clone();

        let node_ref = kameo::spawn(node);

        self.nodes.insert(name, node_ref.clone());

        Ok(node_ref)
    }
}

impl Message<StopNode> for NodeManager {
    type Reply = Result<()>;

    async fn handle(
        &mut self,
        StopNode { name }: StopNode,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        let node = self
            .nodes
            .get(&name)
            .ok_or(anyhow!("node is not started"))?;

        node.stop_gracefully().await?;

        Ok(())
    }
}

impl Message<GetNode> for NodeManager {
    type Reply = Result<ActorRef<Node>>;

    async fn handle(
        &mut self,
        GetNode { name }: GetNode,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        let node_ref = self
            .nodes
            .get(&name)
            .ok_or(anyhow!("there is no {} node started", name))?
            .clone();

        Ok(node_ref)
    }
}
