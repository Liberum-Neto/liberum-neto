use std::{collections::HashSet, path::PathBuf};

use libp2p::{
    kad::{
        store::RecordStore, AddProviderError, AddProviderOk, Event, GetProvidersError,
        GetProvidersOk, InboundRequest, ProgressStep, ProviderRecord, PutRecordError, PutRecordOk,
        QueryId, QueryResult, QueryStats, Record, RecordKey,
    },
    swarm::ConnectionId,
    PeerId,
};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

use crate::swarm_runner::{file_share, SwarmContext};

///! The module contains methods to handle Kademlia events
///! Kademlia is used mostly for finding other nodes in the network.

/// Methods on SwarmContext for handling Kademlia
/// On QueryProgressed events generally it is required to remember the query ID
/// from when the query was started to react to the event
impl SwarmContext {
    pub(crate) fn handle_kademlia(&mut self, event: Event) {
        match event {
            Event::OutboundQueryProgressed {
                id,
                result,
                stats,
                step,
            } => {
                self.handle_outbound_query_progressed(id, result, stats, step);
            }
            Event::InboundRequest { request } => {
                self.handle_inound_request(request);
            }
            _ => {}
        }
    }
}

impl SwarmContext {
    fn handle_outbound_query_progressed(
        &mut self,
        id: QueryId,
        result: QueryResult,
        stats: QueryStats,
        step: ProgressStep,
    ) {
        match result {
            // Triggered when a node starts providing an ID
            QueryResult::StartProviding(result) => {
                self.handle_outbound_query_progressed_start_providing(id, result, stats, step);
            }
            // Triggered when a record is put in the DHT
            QueryResult::PutRecord(result) => {
                self.handle_outbound_query_progressed_put_record(id, result, stats, step);
            }
            // Triggered when more providers found or there is no more providers to find
            QueryResult::GetProviders(result) => {
                self.handle_outbound_query_progressed_get_providers(id, result, stats, step);
            }
            _ => {}
        }
    }

    fn handle_inound_request(&mut self, request: InboundRequest) {
        match request {
            // Triggered when the node is asked to put a record into it's store as a part of the DHT
            InboundRequest::PutRecord {
                source,
                connection,
                record,
            } => self.handle_inbound_request_put_record(source, connection, record),
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
    fn handle_outbound_query_progressed_start_providing(
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

        debug!(
            node = self.node_snapshot.name,
            "Start Providing, all known providers:",
        );

        // Respond to the caller of StartProviding
        let sender = self.behaviour.pending_start_providing.remove(&id);
        if let Some(sender) = sender {
            match result {
                Ok(_) => {
                    let _ = sender.send(Ok(()));
                }
                Err(e) => {
                    let _ = sender.send(Err(e.into()));
                }
            }
        } else {
            debug!(qid = format!("{id}"), "Channel closed");
        }
    }

    fn handle_outbound_query_progressed_put_record(
        &mut self,
        id: QueryId,
        result: Result<PutRecordOk, PutRecordError>,
        _stats: QueryStats,
        _step: ProgressStep,
    ) {
        if result.is_ok() {
            info!(
                node = self.node_snapshot.name,
                id = format!("{id:?}"),
                "Put file"
            );
        } else {
            error!(
                node = self.node_snapshot.name,
                id = format!("{id:?}"),
                err = format!("{result:?}"),
                "Failed to put file"
            );
            self.print_neighbours();
        }

        let sender = self.behaviour.pending_publish_file.remove(&id);

        if let Some(sender) = sender {
            match result {
                Ok(_) => {
                    let _ = sender.send(Ok(()));
                }
                Err(e) => {
                    let _ = sender.send(Err(e.into()));
                }
            }
        } else {
            debug!(
                node = self.node_snapshot.name,
                qid = format!("{id}"),
                "Put Record Progressed: Channel closed"
            );
        }
    }

    fn handle_outbound_query_progressed_get_providers(
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
    fn handle_inbound_request_put_record(
        &mut self,
        _source: PeerId,
        _connection: ConnectionId,
        record: Option<Record>,
    ) {
        debug!(node = self.node_snapshot.name, "Kad Received PutRecord");
        if record.is_none() {
            warn!("Received PutRecord with no record");
            return;
        }
        let record = record.unwrap();
        let id = record.key.clone();

        // save record to filem should use a VAULT here instead
        self.put_record_into_vault(record);

        // Start a query to be providing the file ID in kademlia
        let qid = self.swarm.behaviour_mut().kademlia.start_providing(id);
        if let Err(e) = qid {
            debug!(
                node = self.node_snapshot.name,
                err = format!("{e:?}"),
                "Failed to start providing file"
            );
            return;
        }
        let qid = qid.unwrap();

        // We can't await for a response here because it would block the event loop
        let (response_sender, _) = oneshot::channel();
        self.behaviour
            .pending_start_providing
            .insert(qid, response_sender);
    }

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
    pub(crate) fn put_record_into_vault(&mut self, record: Record) {
        let dir = PathBuf::from("FILE_SHARE_SAVED_FILES").join(self.node_snapshot.name.clone());
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(liberum_core::file_id_to_str(record.key.clone()));
        if let Err(e) = std::fs::write(path.clone(), record.value) {
            error!(
                node = self.node_snapshot.name,
                path = format!("{path:?}"),
                err = format!("{e:?}"),
                "Failed to save file"
            );
            return;
        }
        // also should be handled by a VAULT
        self.behaviour.providing.insert(
            record.key.clone(),
            file_share::SharedResource::File { path: path.clone() },
        );
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
