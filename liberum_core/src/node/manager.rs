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

type NamedRefs = HashMap<String, ActorRef<Node>>;

#[derive(Debug)]
pub struct NodeManager {
    nodes: NamedRefs,
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

    fn get_nodes_refs(&self, names: Vec<&str>) -> Result<NamedRefs, NodeManagerError> {
        names
            .into_iter()
            .map(|name| match self.nodes.get(name) {
                Some(node) => Ok((name.to_string(), node.clone())),
                None => Err(NodeManagerError::NotStarted {
                    name: name.to_string(),
                }),
            })
            .collect()
    }

    async fn start_nodes(&mut self, names: Vec<String>) -> Result<NamedRefs, NodeManagerError> {
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
        let node_refs: HashMap<String, ActorRef<Node>> = nodes
            .into_iter()
            .map(|n| {
                let name = n.name.clone();
                let actor_ref = kameo::spawn(n);
                self.nodes.insert(name.clone(), actor_ref.clone());
                (name, actor_ref)
            })
            .collect();

        Ok(node_refs)
    }

    async fn start_all(&mut self) -> Result<NamedRefs, NodeManagerError> {
        let names = self.store.ask(ListNodes {}).send().await?;

        self.start_nodes(names).await
    }

    async fn save_nodes(&self, nodes_refs: NamedRefs) -> Result<(), NodeManagerError> {
        let mut snapshots = Vec::new();

        for (_, n_ref) in nodes_refs {
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

        for (_, n_ref) in nodes_refs {
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

pub struct CreateNodes {
    pub nodes: Vec<Node>,
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

impl Message<CreateNodes> for NodeManager {
    type Reply = Result<(), NodeManagerError>;

    async fn handle(
        &mut self,
        CreateNodes { nodes }: CreateNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        self.store.ask(StoreNodes { nodes }).send().await?;

        Ok(())
    }
}

impl Message<StartNodes> for NodeManager {
    type Reply = Result<NamedRefs, NodeManagerError>;

    async fn handle(
        &mut self,
        StartNodes { names }: StartNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        self.start_nodes(names).await
    }
}

impl Message<StartAll> for NodeManager {
    type Reply = Result<NamedRefs, NodeManagerError>;

    async fn handle(
        &mut self,
        _: StartAll,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        self.start_all().await
    }
}

impl Message<StopNodes> for NodeManager {
    type Reply = Result<(), NodeManagerError>;

    async fn handle(
        &mut self,
        StopNodes { names }: StopNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        let names = names.iter().map(|s| s.as_str()).collect();
        self.stop_nodes(names).await
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
    type Reply = Result<NamedRefs, NodeManagerError>;

    async fn handle(
        &mut self,
        GetNodes { names }: GetNodes,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        let names = names.iter().map(|s| s.as_str()).collect();
        self.get_nodes_refs(names)
    }
}

impl Message<GetAll> for NodeManager {
    type Reply = NamedRefs;

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
