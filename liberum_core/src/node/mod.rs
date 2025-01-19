pub mod manager;
pub mod store;

use crate::modules::Modules;
use crate::swarm_runner;
use crate::vaultv3::{ListObjects, Vaultv3};
use anyhow::{anyhow, Result};
use instrumented_channels::mpsc::Sender;
use instrumented_channels::{mpsc, oneshot};
use kameo::mailbox::bounded::BoundedMailbox;
use kameo::messages;
use kameo::request::MessageSend;
use kameo::{actor::ActorRef, message::Message, Actor};
use liberum_core::node_config::NodeConfig;
use liberum_core::proto::{self, signed::SignedObject, TypedObject};
use liberum_core::proto::{file::PlainFileObject, ResultObject};
use liberum_core::str_to_file_id;
use liberum_core::{DaemonQueryStats, DaemonResponse};
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use manager::NodeManager;
use std::sync::Arc;
use std::{borrow::Borrow, collections::HashSet, fmt, path::PathBuf, str::FromStr};
use swarm_runner::messages::SwarmRunnerMessage;
use tokio::time::Duration;
use tracing::{debug, error, warn};

pub struct Node {
    pub name: String,
    pub keypair: Keypair,
    pub config: NodeConfig,
    pub manager_ref: ActorRef<NodeManager>,
    pub vault_ref: ActorRef<Vaultv3>,
    pub modules: Arc<Modules>,
    // These fields are mandatory, but may be set only after spawning the node, so unwrapping them should be safe from
    // all of the methods:
    pub self_actor_ref: Option<ActorRef<Self>>,
    swarm_sender: Option<mpsc::Sender<SwarmRunnerMessage>>,
}

const DIAL_TIMEOUT: Duration = Duration::from_secs(120);

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
    pub async fn get_providers(
        &mut self,
        obj_id_str: String,
    ) -> Result<(Vec<PeerId>, Option<DaemonQueryStats>)> {
        debug!(node = self.name, "Node got GetProviders");
        let obj_id_kad = str_to_file_id(&obj_id_str)?;
        let obj_id = proto::Hash {
            bytes: obj_id_kad.to_vec().as_slice().try_into()?,
        };
        self.get_providers_inner(&obj_id).await
    }

    async fn get_providers_inner(
        &mut self,
        obj_id: &proto::Hash,
    ) -> Result<(Vec<PeerId>, Option<DaemonQueryStats>)> {
        let (send, recv) = oneshot::channel();

        self.swarm_sender
            .as_mut()
            .unwrap()
            .send(SwarmRunnerMessage::GetProviders {
                obj_id: obj_id.clone(),
                response_sender: send,
            })
            .await?;

        let received = tokio::time::timeout(Duration::from_secs(60), recv).await??;
        let peers: Vec<PeerId> = received
            .0
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let stats = received.1;
        debug!(node = self.name, "Got providers: {peers:?}");
        return Ok((peers, stats));
    }

    /// Message called on the node from the daemon to provide a file.
    /// Calculates the ID of the file and passes it to the swarm. Responds with
    /// the ID of the file using which it can be found.
    #[message]
    pub async fn provide_file(&mut self, path: PathBuf) -> Result<String> {
        let object: TypedObject = PlainFileObject::try_from_path(&path).await?.into();
        let object: TypedObject = SignedObject::sign_ed25519(object, self.keypair.clone())?.into();
        self.provide_object_inner(object).await
    }

    #[message]
    pub async fn get_object(
        &mut self,
        obj_id_str: String,
    ) -> Result<(TypedObject, Option<DaemonQueryStats>)> {
        let obj_id = proto::Hash::try_from(obj_id_str.as_str())?;

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

        let resp = tokio::time::timeout(Duration::from_secs(60), resp_recv).await??;
        let (providers, stats) = resp;
        if providers.is_empty() {
            return Err(anyhow!("Could not find provider for file {obj_id_str}.").into());
        }
        debug!(
            node = self.name,
            obj_id = obj_id_str,
            "Found providers: {providers:?}"
        );
        let providers: Vec<PeerId> = providers
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
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

            match tokio::time::timeout(Duration::from_secs(60), obj_receiver).await? {
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
                    let obj = obj[0].to_owned();
                    let calculated_obj_id = proto::Hash::try_from(&obj)?;
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
                    return Ok((obj, stats));
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
    pub async fn get_addresses(&mut self) -> Result<Vec<Multiaddr>> {
        let (send, recv) = oneshot::channel();

        self.swarm_sender
            .as_mut()
            .unwrap()
            .send(SwarmRunnerMessage::GetAddresses {
                response_sender: send,
            })
            .await?;

        let addrs = tokio::time::timeout(Duration::from_secs(60), recv).await???;
        Ok(addrs)
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
        self.publish_file_inner(path).await
    }
    async fn publish_file_inner(&mut self, path: PathBuf) -> Result<String> {
        // The file has to be read to the memory to be published. There is no other way without
        // a new behaviour kademlia could talk to, which would provide streams of data.
        // (Maybe could be implemented on the existing request_response if it would be generalised more?)
        let object: TypedObject = PlainFileObject::try_from_path(&path).await?.into();
        let object: TypedObject = SignedObject::sign_ed25519(object, self.keypair.clone())?.into();
        self.publish_object_inner(object).await
    }

    #[message]
    pub async fn sign_and_provide_object(&mut self, object: proto::TypedObject) -> Result<String> {
        let object = SignedObject::sign_ed25519(object, self.keypair.clone())?.into();
        self.provide_object_inner(object).await
    }
    async fn provide_object_inner(&mut self, object: proto::TypedObject) -> Result<String> {
        let obj_id = proto::Hash::try_from(&object)?;
        let obj_id_str = obj_id.to_string();

        let (resp_send, mut resp_recv) = oneshot::channel();
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
        resp_recv.close();
        Ok(obj_id_str)
    }
    #[message]
    pub async fn sign_and_publish_object(&mut self, object: TypedObject) -> Result<String> {
        let object = SignedObject::sign_ed25519(object, self.keypair.clone())?.into();
        self.publish_object_inner(object).await
    }

    async fn get_closest_peers(&mut self, object_id: &proto::Hash) -> Result<Vec<PeerId>> {
        let (snd, rcv) = oneshot::channel();
        self.swarm_sender
            .as_mut()
            .unwrap()
            .send(SwarmRunnerMessage::GetClosestPeers {
                obj_id: object_id.clone(),
                response_sender: snd,
            })
            .await?;
        let peers = tokio::time::timeout(DIAL_TIMEOUT, rcv).await??;
        Ok(peers)
    }

    async fn publish_object_inner(&mut self, object: TypedObject) -> Result<String> {
        let obj_id = proto::Hash::try_from(&object)?;
        let obj_id_str = bs58::encode(&obj_id.bytes).into_string();

        let kad_k_parameter: i32 = 20;
        let mut successes = 0;
        let places = self.modules.publish(object.clone()).await?;
        let mut peers = HashSet::new();
        for place in places {
            let p = self.get_closest_peers(&place).await?;
            peers.extend(p);
        }

        for peer in &peers {
            if *peer == self.get_peer_id()? {
                let (snd, mut rcv) = oneshot::channel();
                self.swarm_sender
                    .as_mut()
                    .unwrap()
                    .send(SwarmRunnerMessage::ProvideObject {
                        object: object.clone(),
                        obj_id: obj_id.clone(),
                        response_sender: snd,
                    })
                    .await?;
                rcv.close();
            }

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

            if let Ok(obj) = tokio::time::timeout(Duration::from_secs(60), recv).await? {
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
        }
        if successes >= 1 {
            debug!(
                node = self.name,
                obj_id = obj_id_str,
                "Published object to {successes} other nodes"
            );
            return Ok(obj_id_str);
        }
        Err(anyhow!("Could not publish file"))
    }

    #[message]
    pub async fn get_published_objects(&mut self) -> Result<Vec<proto::Hash>> {
        Ok(self.vault_ref.ask(ListObjects {}).send().await?)
    }

    #[message]
    pub async fn delete_object(&mut self, obj_id_str: String) -> Result<DaemonResponse> {
        let obj_id = proto::Hash::try_from(obj_id_str.as_str())?;
        let (providers, _) = self.get_providers_inner(&obj_id).await?;

        let mut deleted_count: u32 = 0;
        let mut failed_count: u32 = 0;
        let mut deleted_myself = false;
        for peer in &providers {
            if peer == &self.get_peer_id().unwrap() {
                debug!(
                    node = self.name,
                    obj_id = obj_id_str,
                    "Deleting object I'm providing myself"
                );
                let (send, recv) = oneshot::channel();
                self.swarm_sender
                    .as_mut()
                    .unwrap()
                    .send(SwarmRunnerMessage::StopProviding {
                        obj_id: obj_id.clone(),
                        response_sender: send,
                    })
                    .await?;
                let resp = tokio::time::timeout(Duration::from_secs(60), recv).await?;
                if let Err(_) = resp {
                    warn!(
                        node = self.name,
                        "Failed to stop providing the object myself"
                    )
                }
                deleted_myself = true;
                continue;
            }
            let (send, recv) = oneshot::channel();
            self.swarm_sender
                .as_mut()
                .unwrap()
                .send(SwarmRunnerMessage::DeleteObject {
                    obj_id: obj_id.clone(),
                    peer: peer.clone(),
                    response_sender: send,
                })
                .await?;
            let rec = tokio::time::timeout(Duration::from_secs(60), recv).await?;
            match rec {
                Err(e) => {
                    debug!(
                        node = self.name,
                        err = format!("{e}"),
                        asked_node = peer.to_base58(),
                        "Failed to ask to delete"
                    );
                    failed_count += 1;
                }
                Ok(r) => match r {
                    Err(e) => {
                        debug!(
                            node = self.name,
                            err = format!("{e}"),
                            asked_node = peer.to_base58(),
                            "Failed to ask to send Delete Object Query"
                        );
                        failed_count += 1;
                    }
                    Ok(r) => match r.result {
                        Ok(_) => deleted_count += 1,
                        Err(_) => failed_count += 1,
                    },
                },
            }
        }
        Ok(DaemonResponse::ObjectDeleted {
            deleted_myself,
            deleted_count,
            failed_count,
        })
    }
    #[message]
    pub async fn send_query(&mut self, object: TypedObject) -> Result<DaemonResponse> {
        let obj_id = proto::Hash::try_from(&object)?;
        let hashes = self.modules.publish(object.clone()).await?;
        let mut peers = HashSet::new();

        for hash in hashes {
            let (providers, _) = self.get_providers_inner(&hash).await?;
            peers.extend(providers);
        }
        let mut responses = HashSet::new();
        for peer in peers {
            let (snd, rcv) = oneshot::channel();
            self.swarm_sender
                .as_mut()
                .unwrap()
                .send(SwarmRunnerMessage::SendQuery {
                    object: object.clone(),
                    obj_id: obj_id.clone(),
                    peer_id: peer,
                    response_sender: snd,
                })
                .await?;
            if let Ok(resp) = tokio::time::timeout(Duration::from_secs(60), rcv).await?? {
                responses.extend(resp);
            }
        }

        return Ok(DaemonResponse::QueryFinished {
            result: responses.into_iter().collect(),
        });
    }
}

impl Node {
    pub fn builder() -> NodeBuilder {
        NodeBuilder::default()
    }

    async fn start_swarm(&mut self) -> Result<()> {
        self.swarm_sender = Some(
            swarm_runner::run_swarm(
                self.self_actor_ref.as_mut().unwrap().clone(),
                self.vault_ref.clone(),
                self.modules.clone(),
                NodeSnapshot::from(self.borrow()),
            )
            .await,
        );
        debug!(name = self.name, "Node starts");

        Ok(())
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("name", &self.name)
            .field("boostrap_nodes", &self.config.bootstrap_nodes)
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
    config: Option<NodeConfig>,
    manager_ref: Option<ActorRef<NodeManager>>,
    vault_ref: Option<ActorRef<Vaultv3>>,
    self_actor_ref: Option<ActorRef<Node>>,
    swarm_sender: Option<Sender<SwarmRunnerMessage>>,
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self {
            name: None,
            keypair: None,
            config: None,
            manager_ref: None,
            vault_ref: None,
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
        self.config = Some(config);
        self
    }

    pub fn manager_ref(mut self, manager_ref: ActorRef<NodeManager>) -> Self {
        self.manager_ref = Some(manager_ref);
        self
    }

    pub fn vault_ref(mut self, vault_ref: ActorRef<Vaultv3>) -> Self {
        self.vault_ref = Some(vault_ref);
        self
    }

    pub fn from_snapshot(mut self, snapshot: &NodeSnapshot) -> Self {
        self.name = Some(snapshot.name.clone());
        self.keypair = Some(snapshot.keypair.clone());
        self.config = Some(snapshot.config.clone());
        self
    }

    pub fn build(self) -> Result<Node> {
        let vault_ref = self.vault_ref.ok_or(anyhow!("vault ref is required"))?;
        let mut modules = Modules::new();
        modules.install_default(vault_ref.clone());
        let node = Node {
            name: self.name.ok_or(anyhow!("node name is required"))?,
            keypair: self.keypair.ok_or(anyhow!("keypair is required"))?,
            config: self.config.ok_or(anyhow!("config is required"))?,
            manager_ref: self
                .manager_ref
                .ok_or(anyhow!("node manager ref is required"))?,
            vault_ref: vault_ref,
            self_actor_ref: self.self_actor_ref,
            swarm_sender: self.swarm_sender,
            modules: Arc::new(modules),
        };

        Ok(node)
    }

    pub fn build_snapshot(self) -> Result<NodeSnapshot> {
        let snapshot = NodeSnapshot {
            name: self.name.ok_or(anyhow!("node name is required"))?,
            keypair: self.keypair.ok_or(anyhow!("keypair is required"))?,
            config: self.config.unwrap_or(NodeConfig::default()),
        };

        Ok(snapshot)
    }
}

pub struct NodeSnapshot {
    pub name: String,
    pub keypair: Keypair,
    pub config: NodeConfig,
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
            config: value.config.clone(),
        }
    }
}

impl Into<NodeConfig> for &NodeSnapshot {
    fn into(self) -> NodeConfig {
        self.config.clone()
    }
}
