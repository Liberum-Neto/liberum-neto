pub mod config;
pub mod manager;
pub mod store;

use anyhow::{anyhow, bail, Result};
use config::NodeConfig;
use kameo::{actor::ActorRef, message::Message, request::MessageSend, Actor};
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use kameo::mailbox::bounded::BoundedMailbox;
use libp2p::{StreamProtocol, SwarmBuilder};
use manager::NodeManager;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, path::Path};
use tracing::error;
use crate::swarm::start_swarm;

pub struct Node {
    pub name: String,
    pub keypair: Keypair,
    pub bootstrap_nodes: Vec<BootstrapNode>,
    pub manager_ref: Option<ActorRef<NodeManager>>,
    pub external_addresses: Vec<Multiaddr>,
}

impl Actor for Node {
    type Mailbox = BoundedMailbox<Self>;

    async fn on_start(
        &mut self,
        actor_ref: ActorRef<Self>,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        if let Some(manager_ref) = &self.manager_ref {
            let myself = manager_ref
            .ask(manager::GetNodes{names: vec![self.name.clone()]}).send().await?
            .get(&self.name).unwrap().to_owned();
            tokio::spawn(start_swarm(myself));
        }

        Ok(())
    }
}

impl Node {
    const CONFIG_FILE_NAME: &'static str = "config.json";
    const KEY_FILE_NAME: &'static str = "keypair";

    pub fn builder() -> NodeBuilder {
        NodeBuilder::default()
    }

    async fn load(node_dir_path: &Path) -> Result<Node> {
        if !node_dir_path.is_dir() {
            error!(
                dir_path = node_dir_path.display().to_string(),
                "node dir path not a directory"
            );
            bail!("node_dir_path is not a directory");
        }

        let config_path = node_dir_path.join(Node::CONFIG_FILE_NAME);
        let config_bytes = tokio::fs::read(config_path)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not read node config from file"))?;
        let config: NodeConfig = serde_json::from_slice(&config_bytes)
            .inspect_err(|e| error!(err = e.to_string(), "could not parse node config JSON"))?;
        let key_path = node_dir_path.join(Node::KEY_FILE_NAME);
        let key_bytes = tokio::fs::read(key_path)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not read node keypair bytes"))?;
        let keypair = Keypair::from_protobuf_encoding(&key_bytes)?;
        let node_name = node_dir_path
            .file_name()
            .ok_or(anyhow!(
                "incorrect node dir path, it should not end with .."
            ))?
            .to_str()
            .ok_or(anyhow!("node dir path is not valid utf-8 string"))
            .inspect_err(|e| error!(err = e.to_string(), "could not resolve node name"))?
            .to_string();
        let node = Node::builder()
            .name(node_name)
            .config(config)
            .keypair(keypair)
            .build()
            .inspect_err(|e| error!(err = e.to_string(), "error while building node"))?;

        Ok(node)
    }

    async fn save(&self, node_dir_path: &Path) -> Result<()> {
        if !node_dir_path.is_dir() {
            error!("node dir path is not a directory");
            bail!("node_dir_path is not a directory");
        }

        let config: NodeConfig = self.into();
        let config_path = node_dir_path.join(Node::CONFIG_FILE_NAME);
        let key_bytes = self
            .keypair
            .to_protobuf_encoding()
            .inspect_err(|e| error!(err = e.to_string(), "could not convert keypair to bytes"))?;
        let key_path = node_dir_path.join(Node::KEY_FILE_NAME);

        tokio::fs::write(key_path, key_bytes)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not write node keypair"))?;
        tokio::fs::write(config_path, serde_json::to_string(&config)?)
            .await
            .inspect_err(|e| error!(err = e.to_string(), "could not write node config"))?;

        Ok(())
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("name", &self.name)
            .field("boostrap_nodes", &self.bootstrap_nodes)
            .finish()
    }
}

impl Into<NodeConfig> for &Node {
    fn into(self) -> NodeConfig {
        NodeConfig::new(
            self.bootstrap_nodes.clone(),
            self.external_addresses.clone(),
        )
    }
}

impl Clone for Node {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            keypair: self.keypair.clone(),
            bootstrap_nodes: self.bootstrap_nodes.clone(),
            manager_ref: None,
            external_addresses: self.external_addresses.clone(),
        }
    }
}

pub struct GetSnapshot;

impl Message<GetSnapshot> for Node {
    type Reply = Result<Node, kameo::error::Infallible>;

    async fn handle(
        &mut self,
        _: GetSnapshot,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        Ok(self.clone())
    }
}

#[derive(Default)]
pub struct NodeBuilder {
    name: Option<String>,
    keypair: Option<Keypair>,
    bootstrap_nodes: Vec<BootstrapNode>,
    external_addresses: Vec<Multiaddr>,
}

impl NodeBuilder {
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn keypair(mut self, keypair: Keypair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    pub fn config(mut self, config: NodeConfig) -> Self {
        self.bootstrap_nodes = config.bootstrap_nodes;
        self.external_addresses = config.external_addresses;
        self
    }

    pub fn build(self) -> Result<Node> {
        let keypair = self.keypair.ok_or(anyhow!("keypair is required"))?;
        let id = PeerId::from_public_key(&keypair.public());

        return Ok(Node {
            name: self.name.ok_or(anyhow!("node name is required"))?,
            keypair: keypair,
            bootstrap_nodes: self.bootstrap_nodes,
            manager_ref: None,
            external_addresses: self.external_addresses,
        });
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BootstrapNode {
    #[serde(
        serialize_with = "serialize_peer_id",
        deserialize_with = "deserialize_peer_id"
    )]
    id: PeerId,
    addr: Multiaddr,
}

impl BootstrapNode {
    pub fn new(peer_id: PeerId, addr: Multiaddr) -> Self {
        BootstrapNode { id: peer_id, addr }
    }
}

fn serialize_peer_id<S>(peer_id: &PeerId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let peer_id_bytes = peer_id.to_bytes();
    serializer.serialize_bytes(&peer_id_bytes)
}

fn deserialize_peer_id<'de, D>(deserializer: D) -> Result<PeerId, D::Error>
where
    D: Deserializer<'de>,
{
    let peer_id_bytes = <Vec<u8>>::deserialize(deserializer)?;
    PeerId::from_bytes(&peer_id_bytes)
        .map_err(|e| serde::de::Error::custom(format!("could not deserialize PeerId: {}", e)))
}
