pub mod manager;
pub mod store;

use crate::swarm_runner;
use anyhow::{anyhow, Result};
use kameo::mailbox::bounded::BoundedMailbox;
use kameo::messages;
use kameo::{actor::ActorRef, message::Message, Actor};
use liberum_core::node_config::{BootstrapNode, NodeConfig};
use liberum_core::parser;
use liberum_core::proto::{self, TypedObject};
use liberum_core::proto::{PlainFileObject, ResultObject};
use liberum_core::str_to_file_id;
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use manager::NodeManager;
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use swarm_runner::messages::SwarmRunnerMessage;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use tracing::{debug, error};

pub struct Node {
    pub name: String,
    pub keypair: Keypair,
    pub bootstrap_nodes: Vec<BootstrapNode>,
    pub manager_ref: ActorRef<NodeManager>,
    pub external_addresses: Vec<Multiaddr>,
    // These fields are mandatory, but may be set only after spawning the node, so unwrapping them should be safe from
    // all of the methods:
    pub self_actor_ref: Option<ActorRef<Self>>,
    swarm_sender: Option<mpsc::Sender<SwarmRunnerMessage>>,
}

const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

impl Actor for Node {
    type Mailbox = BoundedMailbox<Self>;

    async fn on_start(
        &mut self,
        actor_ref: ActorRef<Self>,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        // This should always be first thing to set self ref, because some methods executed later will assume that
        // this field is Some -- unwrapping this option
        self.self_actor_ref = Some(actor_ref.clone());
        self.start_swarm().await?;

        Ok(())
    }

    async fn on_stop(
        &mut self,
        _: kameo::actor::WeakActorRef<Self>,
        _: kameo::error::ActorStopReason,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        Ok(self
            .swarm_sender
            .as_ref()
            .unwrap()
            .send(SwarmRunnerMessage::Kill)
            .await?)
    }
}

#[messages]
impl Node {
    /// Message called by the swarm when it dies. The node should know about
    /// it and shut down.
    #[message]
    pub async fn swarm_died(&mut self) {
        debug!(node = self.name, "Swarm died! Killing myself!");
        if let Err(e) = self
            .self_actor_ref
            .as_mut()
            .unwrap()
            .stop_gracefully()
            .await
        {
            error!(
                node = self.name,
                err = format!("{e:?}"),
                "Failed to kill node!"
            );
            self.self_actor_ref.as_mut().unwrap().kill();
        }
    }

    /// Message called on the node from the daemon to get the list of providers
    /// of an id. Changes the ID from string to libp2p format and just passes it to the swarm.
    #[message]
    pub async fn get_providers(&mut self, obj_id_str: String) -> Result<HashSet<PeerId>> {
        debug!(node = self.name, "Node got GetProviders");
        let obj_id_kad = str_to_file_id(&obj_id_str)?;
        let obj_id = proto::Hash {
            bytes: obj_id_kad.to_vec().as_slice().try_into()?,
        };
        let (send, recv) = oneshot::channel();

        self.swarm_sender
            .as_mut()
            .unwrap()
            .send(SwarmRunnerMessage::GetProviders {
                obj_id: obj_id,
                response_sender: send,
            })
            .await?;

        if let Ok(received) = recv.await {
            debug!(node = self.name, "Got providers: {received:?}");
            return Ok(received);
        }

        Err(anyhow!("Could not get providers"))
    }

    /// Message called on the node from the daemon to provide a file.
    /// Calculates the ID of the file and passes it to the swarm. Responds with
    /// the ID of the file using which it can be found.
    #[message]
    pub async fn provide_file(&mut self, path: PathBuf) -> Result<String> {
        let (resp_send, resp_recv) = oneshot::channel();

        let object: TypedObject = PlainFileObject::try_from_path(&path).await?.into();
        let obj_id = proto::Hash::try_from(&object).unwrap();

        self.swarm_sender
            .as_mut()
            .unwrap()
            .send(SwarmRunnerMessage::ProvideObject {
                object,
                obj_id: obj_id.clone(),
                response_sender: resp_send,
            })
            .await?;

        resp_recv.await??;
        let obj_id_str = obj_id.to_string();

        Ok(obj_id_str)
    }

    #[message]
    pub async fn download_file(&mut self, obj_id_str: String) -> Result<proto::PlainFileObject> {
        let obj_id = proto::Hash::try_from(&obj_id_str).unwrap();

        // first get the providers of the file
        // Maybe getting the providers could be reused from GetProviders node message handler??
        let (resp_send, resp_recv) = oneshot::channel();

        self.swarm_sender
            .as_mut()
            .unwrap()
            .send(SwarmRunnerMessage::GetProviders {
                obj_id: obj_id.clone(),
                response_sender: resp_send,
            })
            .await?;

        let providers = resp_recv.await?;
        if providers.is_empty() {
            return Err(anyhow!("Could not find provider for file {obj_id_str}.").into());
        }
        debug!(
            node = self.name,
            obj_id = obj_id_str,
            "Found providers: {providers:?}"
        );
        for peer in &providers {
            debug!(
                node = self.name,
                peer_id = peer.to_base58(),
                obj_id = obj_id_str,
                "Trying to download from peer"
            );

            let (obj_sender, obj_receiver) = oneshot::channel();
            let result = self
                .swarm_sender
                .as_mut()
                .unwrap()
                .send(SwarmRunnerMessage::GetObject {
                    obj_id: obj_id.clone(),
                    peer_id: peer.clone(),
                    response_sender: obj_sender,
                });

            if let Err(e) = result.await {
                error!(
                    node = self.name,
                    err = e.to_string(),
                    "Failed to send download file message"
                );
                continue;
            }

            match obj_receiver.await {
                Err(e) => {
                    debug!(
                        node = self.name,
                        from = format!("{peer}"),
                        err = e.to_string(),
                        "Failed to download file"
                    );
                    continue;
                }
                Ok(Err(e)) => {
                    debug!(
                        node = self.name,
                        from = format!("{peer}"),
                        err = e.to_string(),
                        "Failed to download file"
                    );
                    continue;
                }

                Ok(Ok(obj)) => {
                    let calculated_obj_id = proto::Hash::try_from(&obj).unwrap();
                    if obj_id != calculated_obj_id {
                        debug!(
                            node = self.name,
                            from = format!("{peer}"),
                            data = format!("{:?}", &obj.data),
                            "Received wrong file! {} != {obj_id_str}",
                            calculated_obj_id.to_string()
                        );
                        continue;
                    }
                    match parser::parse_typed(obj).await {
                        Ok(parser::ObjectEnum::PlainFile(file)) => return Ok(file),
                        Err(e) => return Err(e),
                        Ok(_) => return Err(anyhow!("Received object was not a file!")),
                    }
                }
            }
        }

        Err(anyhow!("Could not download file"))
    }

    #[message]
    pub fn get_peer_id(&mut self) -> Result<PeerId> {
        Ok(PeerId::from(self.keypair.public()))
    }

    #[message]
    pub async fn dial_peer(&mut self, peer_id: String, peer_addr: String) -> Result<()> {
        let (send, recv) = oneshot::channel();
        let peer_id = PeerId::from_str(&peer_id)?;
        let peer_addr = peer_addr.parse::<Multiaddr>()?;

        self.swarm_sender
            .as_mut()
            .unwrap()
            .send(SwarmRunnerMessage::Dial {
                peer_id,
                peer_addr,
                response_sender: send,
            })
            .await?;
        return match tokio::time::timeout(DIAL_TIMEOUT, recv).await {
            Ok(o) => o?.map_err(|e| e.into()),
            Err(_) => Err(anyhow!("Dial failed: Timeout ({DIAL_TIMEOUT:?}))")),
        };
    }

    #[message]
    pub async fn publish_file(&mut self, path: PathBuf) -> Result<String> {
        // The file has to be read to the memory to be published. There is no other way without
        // a new behaviour kademlia could talk to, which would provide streams of data.
        // (Maybe could be implemented on the existing request_response if it would be generalised more?)
        let object: TypedObject = PlainFileObject::try_from_path(&path).await?.into();
        let obj_id = proto::Hash::try_from(&object).unwrap();
        let obj_id_str = bs58::encode(&obj_id.bytes).into_string();

        let (resp_send, resp_recv) = oneshot::channel();
        self.swarm_sender
            .as_mut()
            .unwrap()
            .send(SwarmRunnerMessage::GetClosestPeers {
                obj_id: obj_id.clone(),
                response_sender: resp_send,
            })
            .await?;

        let peers = resp_recv.await?;
        if peers.is_empty() {
            return Err(anyhow!("Could not find provider for file {obj_id_str}.").into());
        }

        let kad_k_parameter: i32 = 20;
        let mut successes = 0;
        for peer in &peers {
            let (send, recv) = oneshot::channel();
            self.swarm_sender
                .as_mut()
                .unwrap()
                .send(SwarmRunnerMessage::SendObject {
                    object: object.clone(),
                    obj_id: obj_id.clone(),
                    peer_id: peer.clone(),
                    response_sender: send,
                })
                .await?;

            if let Ok(obj) = recv.await {
                match obj {
                    Ok(ResultObject { result: Ok(_) }) => {
                        successes += 1;
                        if successes >= kad_k_parameter {
                            break;
                        }
                    }
                    _ => {
                        continue;
                    }
                }
            }
            if successes >= 1 {
                return Ok(obj_id_str);
            }
        }
        Err(anyhow!("Could not publish file"))
    }

    #[message]
    pub async fn provide_object(&mut self, object: proto::TypedObject) -> Result<String> {
        let obj_id = proto::Hash::try_from(&object).unwrap();
        let obj_id_str = obj_id.to_string();

        let (resp_send, _) = oneshot::channel();
        let _ = self
            .swarm_sender
            .as_mut()
            .unwrap()
            .send(SwarmRunnerMessage::ProvideObject {
                object,
                obj_id: obj_id,
                response_sender: resp_send,
            })
            .await?;

        Ok(obj_id_str)
    }

    #[message]
    pub(crate) fn get_object_from_vault(&mut self, key: proto::Hash) -> Option<proto::TypedObject> {
        let path = PathBuf::from("FILE_SHARE_SAVED_FILES")
            .join(self.name.clone())
            .join(key.to_string());
        match std::fs::read(&path) {
            Ok(data) => Some(TypedObject::try_from(&data).unwrap()),
            Err(e) => {
                error!(
                    node = self.name,
                    key = bs58::encode(&key.bytes).into_string(),
                    err = format!("{e:?}"),
                    path = format!("{path:?}"),
                    "Failed to read file"
                );
                None
            }
        }
    }

    #[message]
    pub async fn put_object_into_vault(&mut self, obj: proto::TypedObject) -> Result<()> {
        let dir = PathBuf::from("FILE_SHARE_SAVED_FILES").join(self.name.clone());
        std::fs::create_dir_all(&dir).ok();
        let obj_id = proto::Hash::try_from(&obj).unwrap();

        let path = dir.join(obj_id.to_string());

        if let Err(e) = std::fs::write(path.clone(), obj.data) {
            error!(
                node = self.name,
                path = format!("{path:?}"),
                err = format!("{e:?}"),
                "Failed to save file"
            );
            return Err(e.into());
        }
        Ok(())
    }
}

impl Node {
    pub fn builder() -> NodeBuilder {
        NodeBuilder::default()
    }

    async fn start_swarm(&mut self) -> Result<()> {
        self.swarm_sender =
            Some(swarm_runner::run_swarm(self.self_actor_ref.as_mut().unwrap().clone()).await);
        debug!(name = self.name, "Node starts");

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

pub struct GetSnapshot;

impl Message<GetSnapshot> for Node {
    type Reply = Result<NodeSnapshot, kameo::error::Infallible>;

    async fn handle(
        &mut self,
        _: GetSnapshot,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        Ok(NodeSnapshot::from(self.borrow()))
    }
}

pub struct NodeBuilder {
    name: Option<String>,
    keypair: Option<Keypair>,
    bootstrap_nodes: Vec<BootstrapNode>,
    external_addresses: Vec<Multiaddr>,
    manager_ref: Option<ActorRef<NodeManager>>,
    self_actor_ref: Option<ActorRef<Node>>,
    swarm_sender: Option<Sender<SwarmRunnerMessage>>,
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self {
            name: None,
            keypair: None,
            bootstrap_nodes: vec![],
            external_addresses: vec![],
            manager_ref: None,
            self_actor_ref: None,
            swarm_sender: None,
        }
    }
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

    pub fn manager_ref(mut self, manager_ref: ActorRef<NodeManager>) -> Self {
        self.manager_ref = Some(manager_ref);
        self
    }

    pub fn bootstrap_nodes(mut self, bootstrap_nodes: Vec<BootstrapNode>) -> Self {
        self.bootstrap_nodes = bootstrap_nodes;
        self
    }

    pub fn external_addresses(mut self, external_addresses: Vec<Multiaddr>) -> Self {
        self.external_addresses = external_addresses;
        self
    }

    pub fn from_snapshot(mut self, snapshot: &NodeSnapshot) -> Self {
        self.name = Some(snapshot.name.clone());
        self.keypair = Some(snapshot.keypair.clone());
        self.bootstrap_nodes = snapshot.bootstrap_nodes.clone();
        self.external_addresses = snapshot.external_addresses.clone();
        self
    }

    pub fn build(self) -> Result<Node> {
        let node = Node {
            name: self.name.ok_or(anyhow!("node name is required"))?,
            keypair: self.keypair.ok_or(anyhow!("keypair is required"))?,
            bootstrap_nodes: self.bootstrap_nodes,
            manager_ref: self
                .manager_ref
                .ok_or(anyhow!("node manager ref is required"))?,
            external_addresses: self.external_addresses,
            self_actor_ref: self.self_actor_ref,
            swarm_sender: self.swarm_sender,
        };

        Ok(node)
    }

    pub fn build_snapshot(self) -> Result<NodeSnapshot> {
        let snapshot = NodeSnapshot {
            name: self.name.ok_or(anyhow!("node name is required"))?,
            keypair: self.keypair.ok_or(anyhow!("keypair is required"))?,
            bootstrap_nodes: self.bootstrap_nodes,
            external_addresses: self.external_addresses,
        };

        Ok(snapshot)
    }
}

pub struct NodeSnapshot {
    pub name: String,
    pub keypair: Keypair,
    pub bootstrap_nodes: Vec<BootstrapNode>,
    pub external_addresses: Vec<Multiaddr>,
}

impl NodeSnapshot {
    pub fn builder() -> NodeBuilder {
        NodeBuilder::default()
    }
}

impl From<&Node> for NodeSnapshot {
    fn from(value: &Node) -> Self {
        Self {
            name: value.name.clone(),
            keypair: value.keypair.clone(),
            bootstrap_nodes: value.bootstrap_nodes.clone(),
            external_addresses: value.external_addresses.clone(),
        }
    }
}

impl Into<NodeConfig> for &NodeSnapshot {
    fn into(self) -> NodeConfig {
        NodeConfig::new(
            self.bootstrap_nodes.clone(),
            self.external_addresses.clone(),
        )
    }
}
