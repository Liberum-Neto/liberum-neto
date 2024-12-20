use super::{
    store::{GetNodeVault, ListNodes, NodeStore, NodeStoreError, StoreNode},
    GetSnapshot, Node, NodeSnapshot,
};
use crate::node::store::LoadNode;
use anyhow::anyhow;
use anyhow::{Error, Result};
use kameo::{
    actor::ActorRef,
    error::{Infallible, SendError},
    mailbox::bounded::BoundedMailbox,
    request::MessageSend,
    spawn, Actor,
};
use liberum_core::node_config::NodeConfig;
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};
use thiserror::Error;
use tracing::{debug, error};

type NodeRefs = HashMap<String, ActorRef<Node>>;

#[derive(Debug)]
pub struct NodeManager {
    nodes: NodeRefs,
    store: ActorRef<NodeStore>,
    actor_ref: Option<ActorRef<NodeManager>>,
}

#[derive(Error, Debug)]
pub enum NodeManagerError {
    #[error("node {name} is already started")]
    AlreadyStarted { name: String },
    #[error("node {name} is started, but should not be")]
    NodeStarted { name: String },
    #[error("node {name} is already stopped")]
    AlreadyStopped { name: String },
    #[error("node {name} is not started")]
    NotStarted { name: String },
    #[error("node store error: {0}")]
    StoreError(NodeStoreError),
    #[error("other node manager error: {0}")]
    OtherError(Error),
}

#[kameo::messages]
impl NodeManager {
    #[message]
    pub fn get_node_store(&self) -> Result<ActorRef<NodeStore>, Infallible> {
        Ok(self.store.clone())
    }

    #[message]
    pub fn is_node_running(&self, name: String) -> bool {
        self.nodes.contains_key(&name)
    }

    #[message]
    pub async fn create_node(
        &mut self,
        node_snapshot: NodeSnapshot,
    ) -> Result<(), NodeManagerError> {
        self.store
            .ask(StoreNode { node_snapshot })
            .send()
            .await
            .map_err(|e| NodeManagerError::OtherError(e.into()))?;

        Ok(())
    }

    #[message]
    pub async fn start_node(&mut self, name: String) -> Result<ActorRef<Node>, NodeManagerError> {
        if self.nodes.contains_key(&name) {
            return Err(NodeManagerError::AlreadyStarted {
                name: name.to_string(),
            });
        }

        let node_snapshot = self
            .store
            .ask(LoadNode { name: name.clone() })
            .send()
            .await?;

        let self_ref = self
            .actor_ref
            .clone()
            .ok_or("expected node manager self ref to be present")
            .map_err(|e| NodeManagerError::OtherError(anyhow!(e)))?;

        let node_vault = self
            .store
            .ask(GetNodeVault { name: name.clone() })
            .send()
            .await
            .map_err(|e| NodeManagerError::OtherError(anyhow!(e)))?;

        let node = Node::builder()
            .from_snapshot(&node_snapshot)
            .manager_ref(self_ref)
            .vault_ref(spawn(node_vault))
            .build()
            .map_err(|e| NodeManagerError::OtherError(e))?;

        let actor_ref = kameo::spawn(node);
        self.nodes.insert(name.clone(), actor_ref.clone());

        let self_ref = self
            .actor_ref
            .as_mut()
            .ok_or(NodeManagerError::OtherError(anyhow!(
                "manager has no actor ref"
            )))?;

        self_ref.link(&actor_ref).await;

        Ok(actor_ref)
    }

    #[message]
    pub async fn start_all(&mut self) -> Result<NodeRefs, NodeManagerError> {
        let names = self.store.ask(ListNodes {}).send().await?;
        let mut named_refs = NodeRefs::new();

        for name in names {
            named_refs.insert(name.clone(), self.start_node(name).await?);
        }

        Ok(named_refs)
    }

    #[message]
    pub async fn get_node(&self, name: String) -> Result<ActorRef<Node>, NodeManagerError> {
        self.get_node_ref(&name)
    }

    #[message]
    pub async fn get_all(&self) -> NodeRefs {
        self.nodes.clone()
    }

    #[message]
    pub async fn get_node_config(&self, name: String) -> Result<NodeConfig, NodeManagerError> {
        let config: NodeConfig = match self.is_node_running(name.clone()) {
            true => {
                let node = self.get_node_ref(&name)?.ask(GetSnapshot).send().await?;
                (&node).into()
            }
            false => {
                self.store
                    .ask(super::store::GetNodeConfig { name })
                    .send()
                    .await?
            }
        };

        Ok(config)
    }

    #[message]
    pub async fn overwrite_node_config(
        &self,
        name: String,
        new_cfg: NodeConfig,
    ) -> Result<(), NodeManagerError> {
        if self.is_node_running(name.clone()) {
            return Err(NodeManagerError::NodeStarted { name });
        }

        self.store
            .ask(super::store::OverwriteNodeConfig { name, new_cfg })
            .send()
            .await?;

        Ok(())
    }

    #[message]
    pub async fn stop_node(&self, name: String) -> Result<(), NodeManagerError> {
        let node_ref = self.get_node_ref(&name)?;
        self.save_node(node_ref.clone()).await?;

        node_ref
            .stop_gracefully()
            .await
            .map_err(|e| NodeManagerError::OtherError(e.into()))?;

        Ok(())
    }

    #[message]
    pub async fn stop_all(&mut self) -> Result<(), NodeManagerError> {
        for name in self.nodes.keys() {
            self.stop_node(name.to_string()).await?;
        }

        Ok(())
    }
}

impl NodeManager {
    pub fn new(store: ActorRef<NodeStore>) -> Self {
        NodeManager {
            nodes: HashMap::new(),
            store,
            actor_ref: None,
        }
    }

    fn get_node_ref(&self, name: &str) -> Result<ActorRef<Node>, NodeManagerError> {
        match self.nodes.get(name) {
            Some(node) => Ok(node.clone()),
            None => Err(NodeManagerError::NotStarted {
                name: name.to_string(),
            }),
        }
    }

    async fn save_node(&self, node_ref: ActorRef<Node>) -> Result<(), NodeManagerError> {
        let snapshot = node_ref
            .ask(super::GetSnapshot)
            .send()
            .await
            .map_err(|e| NodeManagerError::OtherError(e.into()))?;

        self.store
            .ask(StoreNode {
                node_snapshot: snapshot,
            })
            .send()
            .await
            .map_err(|e| NodeManagerError::OtherError(e.into()))?;

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
        &mut self,
        _: kameo::actor::WeakActorRef<Self>,
        _: kameo::error::ActorStopReason,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        self.stop_all().await?;
        Ok(())
    }

    async fn on_link_died(
        &mut self,
        _: kameo::actor::WeakActorRef<Self>,
        id: kameo::actor::ActorID,
        _: kameo::error::ActorStopReason,
    ) -> std::result::Result<Option<kameo::error::ActorStopReason>, kameo::error::BoxError> {
        debug!(id = id.to_string(), "node died");
        let name = self
            .nodes
            .keys()
            .filter_map(|k| {
                if self.nodes.get(k.as_str())?.id() == id {
                    Some(k)
                } else {
                    None
                }
            })
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        let name = name
            .first()
            .ok_or(anyhow!("there is no such node started"))?;
        self.nodes.remove(name);

        Ok(None)
    }
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
