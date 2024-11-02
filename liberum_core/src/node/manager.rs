use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use kameo::{
    actor::ActorRef,
    mailbox::bounded::BoundedMailbox,
    message::Message,
    request::MessageSend,
    Actor,
};

use crate::node::store::LoadNodes;

use super::{
    store::{ListNodes, NodeStore, StoreNodes},
    Node,
};

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

    fn get_nodes_refs(&self, names: Vec<&String>) -> Result<Vec<&ActorRef<Node>>> {
        let node_refs = names.into_iter()
            .map(|name| {
                self.nodes.get(name).ok_or(anyhow!("node {name} is not started"))
            })
            .collect::<Result<Vec<&ActorRef<Node>>>>()?;

        Ok(node_refs)
    }

    async fn save_nodes(&self, nodes_refs: Vec<&ActorRef<Node>>) -> Result<()> {
        let mut snapshots = Vec::new();

        for n_ref in nodes_refs {
            snapshots.push(n_ref.ask(super::GetSnapshot).send().await?);
        }

        self.store.ask(StoreNodes{nodes: snapshots});

        Ok(())
    }

    async fn stop_nodes(&self, names: Vec<&String>) -> Result<()> {
        let nodes_refs = self.get_nodes_refs(names)?;
        self.save_nodes(nodes_refs.clone()).await?;

        for n_ref in nodes_refs {
            n_ref.stop_gracefully().await?;
        }

        Ok(())
    }

    async fn stop_all(&self) -> Result<()> {
        let names = self.nodes.keys().collect::<Vec<&String>>();
        self.stop_nodes(names).await?;

        Ok(())
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

    async fn on_stop(
        self,
        _: kameo::actor::WeakActorRef<Self>,
        _: kameo::error::ActorStopReason,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        self.stop_all().await?;
        Ok(())
    }
}

pub struct StartNodes {
    pub names: Vec<String>,
}
pub struct StartAll;

pub struct StopNodes {
    pub names: Vec<String>,
}
pub struct StopAll;

pub struct GetNodes {
    pub names: Vec<String>,
}
struct GetAll;

impl Message<StartNodes> for NodeManager {
    type Reply = Result<Vec<ActorRef<Node>>>;

    async fn handle(
        &mut self,
        StartNodes { names }: StartNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        for name in &names {
            if self.nodes.contains_key(name) {
                bail!("node {name} is already started");
            }
        }

        let mut nodes: Vec<Node> = self.store.ask(LoadNodes { names }).send().await?;

        nodes
            .iter_mut()
            .for_each(|n| n.manager_ref = self.actor_ref.clone());
        let node_refs: Vec<ActorRef<Node>> = nodes
            .into_iter()
            .map(|n| {
                let name = n.name.clone();
                let actor_ref = kameo::spawn(n);
                self.nodes.insert(name, actor_ref.clone());
                actor_ref
            })
            .collect();

        Ok(node_refs)
    }
}

impl Message<StartAll> for NodeManager {
    type Reply = Result<Vec<ActorRef<Node>>>;

    async fn handle(
        &mut self,
        _: StartAll,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        let names = self.store.ask(ListNodes {}).send().await?;
        let nodes = self.store.ask(LoadNodes { names }).send().await?;
        let nodes = nodes
            .into_iter()
            .map(|node| {
                let name = node.name.clone();
                let actor_ref = kameo::spawn(node);
                self.nodes.insert(name, actor_ref.clone());
                actor_ref
            })
            .collect();

        Ok(nodes)
    }
}

impl Message<StopNodes> for NodeManager {
    type Reply = Result<()>;

    async fn handle(
        &mut self,
        StopNodes { names }: StopNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        for name in &names {
            self.nodes
                .get(name)
                .ok_or(anyhow!("node {name} is not started"))?
                .stop_gracefully()
                .await?;
        }

        Ok(())
    }
}

impl Message<StopAll> for NodeManager {
    type Reply = Result<()>;

    async fn handle(
        &mut self,
        _: StopAll,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        self.stop_all().await?;
        Ok(())
    }
}

impl Message<GetNodes> for NodeManager {
    type Reply = Result<Vec<ActorRef<Node>>>;

    async fn handle(
        &mut self,
        GetNodes { names }: GetNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        let node_refs: Result<Vec<ActorRef<Node>>> = names
            .iter()
            .map(|name| {
                Ok(self
                    .nodes
                    .get(name)
                    .ok_or(anyhow!("there is no {} node started", name))?
                    .clone())
            })
            .collect();

        node_refs
    }
}

impl Message<GetAll> for NodeManager {
    type Reply = HashMap<String, ActorRef<Node>>;

    async fn handle(
        &mut self,
        _: GetAll,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        self.nodes.clone()
    }
}
