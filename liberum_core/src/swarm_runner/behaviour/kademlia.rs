use std::time::Duration;

use crate::{
    swarm_runner::{object_sender, SwarmContext},
    vault::{LoadObject, StoreObject},
};
use anyhow::Result;
use kameo::request::MessageSend;
use liberum_core::{parser::ObjectEnum, proto, DaemonQueryStats};
use libp2p::{
    kad::{
        store::RecordStore, AddProviderError, AddProviderOk, Event, GetClosestPeersResult,
        GetProvidersError, GetProvidersOk, InboundRequest, ProgressStep, ProviderRecord, QueryId,
        QueryResult, QueryStats, RecordKey,
    },
    PeerId,
};

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
        if let Some(sender) = self.behaviour.pending_inner_start_providing.remove(&id) {
            match result {
                Ok(_) => {
                    let _ = sender.send(Ok(()));
                }
                Err(e) => {
                    let _ = sender.send(Err(e.into()));
                }
            }
        } else if let Some((object_id, response_channel)) =
            self.behaviour.pending_outer_start_providing.remove(&id)
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
                if let Some(sender) = self.behaviour.pending_inner_get_closest_peers.remove(&id) {
                    let peers: Vec<PeerId> = closest_peers
                        .peers
                        .iter()
                        .map(|p| p.peer_id.clone())
                        .collect();
                    let _ = sender.send(peers).inspect_err(|e| {
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
        let query_stats = if let Some(d) = _stats.duration() {
            Some(DaemonQueryStats {
                query_duration: d,
                total_requests: _stats.num_requests(),
            })
        } else {
            None
        };

        match result {
            Ok(GetProvidersOk::FoundProviders { key: _, providers }) => {
                debug!(
                    "get providers pending:{}, last step?: {}",
                    _stats.num_pending(),
                    _step.last
                );
                debug!(
                    node = self.node_snapshot.name,
                    "some providers found {}",
                    providers.len()
                );
                if providers.len() == 0 {
                    return;
                }
                if let Some(sender) = self.behaviour.pending_inner_get_providers.remove(&id) {
                    let mut nodes = sender.0;

                    nodes.append(&mut providers.into_iter().collect());
                    if !_step.last {
                        self.behaviour
                            .pending_inner_get_providers
                            .insert(id, (nodes, sender.1));
                        return;
                    }

                    let _ = sender.1.send((nodes, query_stats)).inspect_err(|e| {
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
                if let Some(sender) = self.behaviour.pending_inner_get_providers.remove(&id) {
                    let _ = sender.1.send((sender.0, query_stats)).inspect_err(|e| {
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
        debug!(
            node = self.node_snapshot.name,
            closer = _num_closer_peers,
            providers = _num_provider_peers,
            "Kad Received GetProvider"
        )
    }
}

/// Utility related to the Kademlia behaviour
impl SwarmContext {
    pub async fn get_object_from_vault(
        &mut self,
        obj_id: proto::Hash,
    ) -> Option<proto::TypedObject> {
        let obj = self
            .vault_ref
            .ask(LoadObject {
                hash: obj_id.clone(),
            })
            .send()
            .await
            .unwrap();

        match obj {
            Some(obj) => match obj {
                ObjectEnum::Typed(typed) => Some(typed),
                _ => None,
            },
            None => None,
        }
    }

    pub async fn put_object_into_vault(&mut self, obj: proto::TypedObject) -> Result<()> {
        let obj_id: proto::Hash = proto::Hash::try_from(&obj).unwrap();

        self.vault_ref
            .ask(StoreObject {
                hash: obj_id,
                object: ObjectEnum::Typed(obj),
            })
            .send()
            .await?;

        Ok(())
    }

    pub(crate) fn print_providers(&mut self, obj_id_kad: &RecordKey) {
        debug!(
            node = self.node_snapshot.name,
            obj_id_kad = bs58::encode(obj_id_kad.to_vec()).into_string(),
            "Providers:"
        );
        for p in self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .providers(obj_id_kad)
            .iter()
        {
            debug!(
                node = self.node_snapshot.name,
                provider_id = p.provider.to_base58(),
                "Provider"
            );
        }
    }
}
