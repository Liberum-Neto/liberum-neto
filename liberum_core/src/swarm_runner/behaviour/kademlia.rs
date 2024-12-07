use crate::swarm_runner::{object_sender, SwarmContext};
use anyhow::Result;
use liberum_core::proto;
use libp2p::{
    kad::{
        store::RecordStore, AddProviderError, AddProviderOk, Event, GetClosestPeersResult,
        GetProvidersError, GetProvidersOk, InboundRequest, ProgressStep, ProviderRecord, QueryId,
        QueryResult, QueryStats, RecordKey,
    },
    PeerId,
};
use std::{collections::HashSet, path::PathBuf};

use tracing::{debug, error, info, warn};

///! The module contains methods to handle Kademlia events
///! Kademlia is used mostly for finding other nodes in the network.

/// Methods on SwarmContext for handling Kademlia
/// On QueryProgressed events generally it is required to remember the query ID
/// from when the query was started to react to the event
impl SwarmContext {
    pub(crate) async fn handle_kademlia(&mut self, event: Event) {
        match event {
            Event::OutboundQueryProgressed {
                id,
                result,
                stats,
                step,
            } => {
                self.handle_outbound_query_progressed(id, result, stats, step)
                    .await;
            }
            Event::InboundRequest { request } => {
                self.handle_inound_request(request);
            }
            _ => {}
        }
    }
}

impl SwarmContext {
    async fn handle_outbound_query_progressed(
        &mut self,
        id: QueryId,
        result: QueryResult,
        stats: QueryStats,
        step: ProgressStep,
    ) {
        match result {
            // Triggered when a node starts providing an ID
            QueryResult::StartProviding(result) => {
                self.handle_outbound_query_progressed_start_providing(id, result, stats, step)
                    .await;
            }
            QueryResult::GetClosestPeers(result) => {
                self.handle_outbound_query_progressed_get_closest_peers(id, result, stats, step)
                    .await;
            }
            // Triggered when more providers found or there is no more providers to find
            QueryResult::GetProviders(result) => {
                self.handle_outbound_query_progressed_get_providers(id, result, stats, step)
                    .await;
            }
            _ => {}
        }
    }

    fn handle_inound_request(&mut self, request: InboundRequest) {
        match request {
            // Triggered when the node is asked to add a provider for a record
            InboundRequest::AddProvider { record } => {
                self.handle_inbound_request_add_provider(record)
            }
            // Triggered when the node is asked to get providers for a record
            InboundRequest::GetProvider {
                num_closer_peers,
                num_provider_peers,
            } => self.handle_inbound_request_get_provider(num_closer_peers, num_provider_peers),
            _ => {}
        }
    }
}

/// Methods to handle Kademlia OutboundQueryProgressed events
impl SwarmContext {
    async fn handle_outbound_query_progressed_start_providing(
        &mut self,
        id: QueryId,
        result: Result<AddProviderOk, AddProviderError>,
        _stats: QueryStats,
        _step: ProgressStep,
    ) {
        // result is matched two times to trace the result no matter
        // if the response is sent via the oneshot channel or not
        if result.is_ok() {
            let result = result.clone();
            info!(
                node = self.node_snapshot.name,
                id = format!(
                    "{}",
                    liberum_core::file_id_to_str(result.unwrap().key.clone())
                ),
                "Started providing file"
            );
        } else {
            debug!(
                node = self.node_snapshot.name,
                id = format!("{id:?}"),
                "Failed to start providing file"
            );
        }

        // Respond to the caller of StartProviding
        if let Some(sender) = self.behaviour.pending_start_providing.remove(&id) {
            match result {
                Ok(_) => {
                    let _ = sender.send(Ok(()));
                }
                Err(e) => {
                    let _ = sender.send(Err(e.into()));
                }
            }
        } else if let Some((object_id, response_channel)) =
            self.behaviour.pending_object_start_providing.remove(&id)
        {
            let _ = self.swarm.behaviour_mut().object_sender.send_response(
                response_channel,
                object_sender::ObjectResponse {
                    object: proto::ResultObject { result: Ok(()) }.into(),
                    object_id,
                },
            );
        }
    }

    async fn handle_outbound_query_progressed_get_closest_peers(
        &mut self,
        id: QueryId,
        result: GetClosestPeersResult,
        _stats: QueryStats,
        _step: ProgressStep,
    ) {
        match result {
            Ok(closest_peers) => {
                if let Some(sender) = self.behaviour.pending_get_closest_peers.remove(&id) {
                    let peers: Vec<PeerId> = closest_peers
                        .peers
                        .iter()
                        .map(|p| p.peer_id.clone())
                        .collect();
                    let _ = sender.send(HashSet::from_iter(peers)).inspect_err(|e| {
                        debug!(
                            node = self.node_snapshot.name,
                            qid = format!("{id}"),
                            err = format!("{e:?}"),
                            "Channel closed"
                        )
                    });
                }
            }
            Err(e) => {
                error!(
                    node = self.node_snapshot.name,
                    id = format!("{id:?}"),
                    err = format!("{e:?}"),
                    "Failed to get closest peers"
                );
            }
        }
    }

    async fn handle_outbound_query_progressed_get_providers(
        &mut self,
        id: QueryId,
        result: Result<GetProvidersOk, GetProvidersError>,
        _stats: QueryStats,
        _step: ProgressStep,
    ) {
        match result {
            Ok(GetProvidersOk::FoundProviders { key: _, providers }) => {
                if let Some(sender) = self.behaviour.pending_get_providers.remove(&id) {
                    let _ = sender.send(providers).inspect_err(|e| {
                        debug!(
                            node = self.node_snapshot.name,
                            qid = format!("{id}"),
                            err = format!("{e:?}"),
                            "Channel closed"
                        )
                    });
                    // Stop the query after we got *some* providers. TODO: Leave the decission when to stop to someone requesting the query
                    // like the node actor. This works for now.
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .query_mut(&id)
                        .unwrap()
                        .finish();
                }
            }
            Ok(GetProvidersOk::FinishedWithNoAdditionalRecord { closest_peers: _ }) => {
                debug!(
                    node = self.node_snapshot.name,
                    "Get providers didn't find any new records"
                );
                if let Some(sender) = self.behaviour.pending_get_providers.remove(&id) {
                    let _ = sender.send(HashSet::new()).inspect_err(|e| {
                        debug!(
                            qid = format!("{id}"),
                            err = format!("{e:?}"),
                            "Channel closed"
                        )
                    });
                }
            }
            Err(e) => {
                error!(
                    node = self.node_snapshot.name,
                    id = format!("{id:?}"),
                    err = format!("{e:?}"),
                    "Failed to get providers"
                );
            }
        }
    }
}

/// Methods to handle Kademlia InboundRequest events
impl SwarmContext {
    fn handle_inbound_request_add_provider(&mut self, record: Option<ProviderRecord>) {
        match record {
            Some(record) => {
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .store_mut()
                    .add_provider(record.clone())
                    .ok(); // TODO What if the providers amount is exceeded? How to ensure only the closest one are kept?
                info!(
                    node = self.node_snapshot.name,
                    provider = record.provider.to_base58(),
                    record = bs58::encode(&record.key).into_string(),
                    "Received AddProvider"
                );
                self.print_providers(&record.key);
            }
            None => {
                warn!(
                    node = self.node_snapshot.name,
                    "Received AddProvider with no record"
                );
            }
        }
    }

    fn handle_inbound_request_get_provider(
        &mut self,
        _num_closer_peers: usize,
        _num_provider_peers: usize,
    ) {
        debug!(node = self.node_snapshot.name, "Kad Received GetProvider")
    }
}

/// Utility related to the Kademlia behaviour
impl SwarmContext {
    pub fn get_object_from_vault(&mut self, key: proto::Hash) -> Option<proto::TypedObject> {
        let path = PathBuf::from("FILE_SHARE_SAVED_FILES")
            .join(self.node_snapshot.name.clone())
            .join(liberum_core::file_id_hash_to_str(&key.bytes.clone()));

        match std::fs::read(&path) {
            Ok(data) => {
                debug!("Getting object from vault: {:?}", data);
                let obj = bincode::deserialize::<proto::TypedObject>(&data).unwrap();
                Some(obj)
            }
            Err(e) => {
                error!(
                    node = self.node_snapshot.name,
                    key = bs58::encode(&key.bytes).into_string(),
                    err = format!("{e:?}"),
                    path = format!("{path:?}"),
                    "Failed to read file"
                );
                None
            }
        }
    }
    pub async fn put_object_into_vault(&mut self, obj: proto::TypedObject) -> Result<()> {
        let dir = PathBuf::from("FILE_SHARE_SAVED_FILES").join(self.node_snapshot.name.clone());
        std::fs::create_dir_all(&dir).ok();
        let id = proto::Hash::try_from(&obj).unwrap();
        let data = bincode::serialize(&obj).unwrap();
        debug!("Putting file to vault: {:?}", data);

        let path = dir.join(liberum_core::file_id_hash_to_str(&id.bytes));

        if let Err(e) = std::fs::write(path.clone(), data) {
            error!(
                node = self.node_snapshot.name,
                path = format!("{path:?}"),
                err = format!("{e:?}"),
                "Failed to save file"
            );
            return Err(e.into());
        }
        Ok(())
    }

    pub(crate) fn print_providers(&mut self, key: &RecordKey) {
        debug!(
            node = self.node_snapshot.name,
            record = bs58::encode(key.to_vec()).into_string(),
            "Providers:"
        );
        for p in self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .providers(key)
            .iter()
        {
            debug!(
                node = self.node_snapshot.name,
                provider = p.provider.to_base58(),
                "Provider"
            );
        }
    }
}
