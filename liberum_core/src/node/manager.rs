use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use anyhow::{Error, Result};
use kameo::{
    actor::ActorRef, error::SendError, mailbox::bounded::BoundedMailbox, message::Message,
    request::MessageSend, Actor,
};
use thiserror::Error;

use crate::node::store::LoadNodes;

use super::{
    store::{ListNodes, NodeStore, NodeStoreError, StoreNodes},
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

    fn get_nodes_refs(&self, names: Vec<&str>) -> Result<Vec<&ActorRef<Node>>, NodeManagerError> {
        let node_refs = names
            .into_iter()
            .map(|name| {
                self.nodes.get(name).ok_or(NodeManagerError::NotStarted {
                    name: name.to_string(),
                })
            })
            .collect::<Result<Vec<&ActorRef<Node>>, NodeManagerError>>()?;

        Ok(node_refs)
    }

    fn get_nodes_refs_owned(
        &self,
        names: Vec<&str>,
    ) -> Result<Vec<ActorRef<Node>>, NodeManagerError> {
        let node_refs = self
            .get_nodes_refs(names)?
            .into_iter()
            .map(|n_ref| n_ref.clone())
            .collect::<Vec<ActorRef<Node>>>();

        Ok(node_refs)
    }

    async fn save_nodes(&self, nodes_refs: Vec<&ActorRef<Node>>) -> Result<(), NodeManagerError> {
        let mut snapshots = Vec::new();

        for n_ref in nodes_refs {
            let snapshot = n_ref
                .ask(super::GetSnapshot)
                .send()
                .await
                .map_err(|e| NodeManagerError::OtherError(e.into()))?;
            snapshots.push(snapshot);
        }

        self.store
            .ask(StoreNodes { nodes: snapshots })
            .send()
            .await?;

        Ok(())
    }

    async fn stop_nodes(&self, names: Vec<&str>) -> Result<(), NodeManagerError> {
        let nodes_refs = self.get_nodes_refs(names)?;
        self.save_nodes(nodes_refs.clone()).await?;

        for n_ref in nodes_refs {
            n_ref
                .stop_gracefully()
                .await
                .map_err(|e| NodeManagerError::OtherError(e.into()))?;
        }

        Ok(())
    }

    async fn stop_all(&self) -> Result<(), NodeManagerError> {
        let names = self.nodes.keys().map(|k| k.as_str()).collect::<Vec<&str>>();
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
    type Reply = Result<Vec<ActorRef<Node>>, NodeManagerError>;

    async fn handle(
        &mut self,
        StartNodes { names }: StartNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        for name in &names {
            if self.nodes.contains_key(name) {
                return Err(NodeManagerError::AlreadyStarted {
                    name: name.to_string(),
                });
            }
        }

        let mut nodes = self.store.ask(LoadNodes { names }).send().await?;

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
    type Reply = Result<Vec<ActorRef<Node>>, NodeManagerError>;

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
    type Reply = Result<(), NodeManagerError>;

    async fn handle(
        &mut self,
        StopNodes { names }: StopNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        for name in &names {
            self.nodes
                .get(name)
                .ok_or(NodeManagerError::NotStarted {
                    name: name.to_string(),
                })?
                .stop_gracefully()
                .await?;
        }

        Ok(())
    }
}

impl Message<StopAll> for NodeManager {
    type Reply = Result<(), NodeManagerError>;

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
    type Reply = Result<Vec<ActorRef<Node>>, NodeManagerError>;

    async fn handle(
        &mut self,
        GetNodes { names }: GetNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        let names = names.iter().map(|n| n.as_str()).collect::<Vec<&str>>();
        self.get_nodes_refs_owned(names)
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

#[derive(Error, Debug)]
pub enum NodeManagerError {
    #[error("node {name} is already started")]
    AlreadyStarted { name: String },
    #[error("node {name} is already stopped")]
    AlreadyStopped { name: String },
    #[error("node {name} is not started")]
    NotStarted { name: String },
    #[error("node store error: {0}")]
    StoreError(NodeStoreError),
    #[error("other node manager error: {0}")]
    OtherError(Error),
}

impl From<NodeStoreError> for NodeManagerError {
    fn from(value: NodeStoreError) -> Self {
        NodeManagerError::StoreError(value)
    }
}

impl From<kameo::error::Infallible> for NodeManagerError {
    fn from(value: kameo::error::Infallible) -> Self {
        NodeManagerError::OtherError(value.into())
    }
}

impl<T, U> From<SendError<T, U>> for NodeManagerError
where
    T: 'static + Send + Sync,
    U: 'static + Send + Sync + Debug + Display,
    NodeManagerError: From<U>,
{
    fn from(value: SendError<T, U>) -> Self {
        match value {
            SendError::HandlerError(e) => e.into(),
            value => NodeManagerError::OtherError(value.into()),
        }
    }
}
