use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use anyhow::anyhow;
use libp2p::{
    kad::{
        self, store::RecordStore, AddProviderError, AddProviderOk, KBucketDistance, PeerRecord,
        ProgressStep, ProviderRecord, PutRecordError, PutRecordOk, QueryId, QueryStats, RecordKey,
    },
    swarm::ConnectionId,
    PeerId,
};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

use crate::swarm_runner::{file_share, SwarmContext};

/// Methods on SwarmContext for handling Kademlia
/// On QueryProgressed events generally it is required to remember the query ID
/// from when the query was started to react to the event
impl SwarmContext {
    pub(crate) fn handle_kademlia(&mut self, event: kad::Event) {
        match event {
            // Triggered when a node starts providing an ID
            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::StartProviding(result),
                stats,
                step,
            } => self.handle_outbound_query_progressed_start_providing(id, result, stats, step),

            // Triggered when a record is put in the DHT
            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::PutRecord(result),
                stats,
                step,
            } => self.handle_outbound_query_progressed_put_record(id, result, stats, step),

            // Triggered when some providers are found for a file ID
            kad::Event::OutboundQueryProgressed {
                id,
                result:
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                        key,
                        providers,
                    })),
                stats,
                step,
            } => self.handle_outbound_query_progressed_get_providers_found(
                id, key, providers, stats, step,
            ),

            // Triggered when no providers are found for a file ID
            kad::Event::OutboundQueryProgressed {
                id,
                result:
                    kad::QueryResult::GetProviders(Ok(
                        kad::GetProvidersOk::FinishedWithNoAdditionalRecord { closest_peers },
                    )),
                stats,
                step,
            } => self.handle_outbound_query_progressed_get_providers_no_additional_record(
                id,
                closest_peers,
                stats,
                step,
            ),

            // Triggered when the node is asked to put a record into it's store as a part of the DHT
            kad::Event::InboundRequest {
                request:
                    kad::InboundRequest::PutRecord {
                        source,
                        connection,
                        record,
                    },
            } => self.handle_inbound_request_put_record(source, connection, record),

            // Triggered when a record of ID is found in the DHT. The query must be finished manually
            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))),
                stats,
                step,
            } => self.handle_outbound_query_progressed_get_record_found(id, record, stats, step),

            // Triggered when no record was found
            kad::Event::OutboundQueryProgressed {
                id,
                result:
                    kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FinishedWithNoAdditionalRecord {
                        cache_candidates,
                    })),
                stats,
                step,
            } => self.handle_outbound_query_progressed_get_record_no_additional_record(
                id,
                cache_candidates,
                stats,
                step,
            ),

            //Debug Prints only from this point
            kad::Event::InboundRequest {
                request: kad::InboundRequest::AddProvider { record },
            } => self.handle_inbound_request_add_provider(record),

            kad::Event::InboundRequest {
                request:
                    kad::InboundRequest::GetProvider {
                        num_closer_peers,
                        num_provider_peers,
                    },
            } => self.handle_inbound_request_get_provider(num_closer_peers, num_provider_peers),
            _ => {}
        }
    }
}

/// Implement event handlers for Kademlia OutboundQueryProgressed events
impl SwarmContext {
    fn handle_outbound_query_progressed_start_providing(
        &mut self,
        id: kad::QueryId,
        result: Result<AddProviderOk, AddProviderError>,
        _stats: kad::QueryStats,
        _step: kad::ProgressStep,
    ) {
        // result is matched two times to trace the result no matter
        // if the response is sent via the oneshot channel or not
        if result.is_ok() {
            let result = result.clone();
            info!(
                node = self.node.name,
                id = format!(
                    "{}",
                    liberum_core::file_id_to_str(result.unwrap().key.clone())
                ),
                "Started providing file"
            );
        } else {
            debug!(
                node = self.node.name,
                id = format!("{id:?}"),
                "Failed to start providing file"
            );
        }

        debug!(
            node = self.node.name,
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
                    let _ = sender.send(Err(anyhow!(e)));
                }
            }
        } else {
            debug!(qid = format!("{id}"), "Channel closed");
        }
    }

    fn handle_outbound_query_progressed_put_record(
        &mut self,
        id: kad::QueryId,
        result: Result<PutRecordOk, PutRecordError>,
        _stats: kad::QueryStats,
        _step: kad::ProgressStep,
    ) {
        if result.is_ok() {
            info!(node = self.node.name, id = format!("{id:?}"), "Put file");
        } else {
            error!(
                node = self.node.name,
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
                    let _ = sender.send(Err(anyhow!(e)));
                }
            }
        } else {
            debug!(
                node = self.node.name,
                qid = format!("{id}"),
                "Put Record Progressed: Channel closed"
            );
        }
    }

    fn handle_outbound_query_progressed_get_providers_found(
        &mut self,
        id: kad::QueryId,
        _key: RecordKey,
        providers: HashSet<PeerId>,
        _stats: kad::QueryStats,
        _step: kad::ProgressStep,
    ) {
        if let Some(sender) = self.behaviour.pending_get_providers.remove(&id) {
            let _ = sender.send(providers).inspect_err(|e| {
                debug!(
                    node = self.node.name,
                    qid = format!("{id}"),
                    err = format!("{e:?}"),
                    "Channel closed"
                )
            });
            // Finish the query to prevent from triggering FinishedWithNoAdditionalRecord
            self.swarm
                .behaviour_mut()
                .kademlia
                .query_mut(&id)
                .unwrap()
                .finish();
        }
    }

    fn handle_outbound_query_progressed_get_providers_no_additional_record(
        &mut self,
        id: kad::QueryId,
        _closest_peers: Vec<PeerId>,
        _stats: kad::QueryStats,
        _step: kad::ProgressStep,
    ) {
        debug!(
            node = self.node.name,
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

    fn handle_outbound_query_progressed_get_record_found(
        &mut self,
        id: QueryId,
        record: PeerRecord,
        _stats: QueryStats,
        _step: ProgressStep,
    ) {
        debug!("Found record in DHT {:?}", id);
        if let Some(sender) = self.behaviour.pending_download_file_dht.remove(&id) {
            let _ = sender.send(record.record.value);
            self.swarm
                .behaviour_mut()
                .kademlia
                .query_mut(&id)
                .unwrap()
                .finish();
        }
    }

    fn handle_outbound_query_progressed_get_record_no_additional_record(
        &mut self,
        id: QueryId,
        _cache_candidates: BTreeMap<KBucketDistance, PeerId>,
        _stats: QueryStats,
        _step: ProgressStep,
    ) {
        debug!("Didn't find record in DHT {:?}", id);
        // inform the caller that the file was not found
        if let Some(sender) = self.behaviour.pending_download_file_dht.remove(&id) {
            let _ = sender.send(Vec::new());
        }
    }
}

/// Implement event handlers for Kademlia InboundRequest events
impl SwarmContext {
    fn handle_inbound_request_put_record(
        &mut self,
        _source: PeerId,
        _connection: ConnectionId,
        record: Option<kad::Record>,
    ) {
        debug!(node = self.node.name, "Kad Received PutRecord");
        if record.is_none() {
            warn!("Received PutRecord with no record");
            return;
        }
        let record = record.unwrap();

        let r = self
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .put(record.clone());
        if r.is_err() {
            debug!(
                node = self.node.name,
                err = format!("{r:?}"),
                "Kad Failed to put record"
            );
            return;
        }

        let id = record.key.clone();

        // save record to filem should use a VAULT here instead
        self.put_record_into_vault(record);

        // Start a query to be providing the file ID in kademlia
        let qid = self
            .swarm
            .behaviour_mut()
            .kademlia
            .start_providing(id.clone());
        if qid.is_err() {
            debug!(
                node = self.node.name,
                err = format!("{qid:?}"),
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
                    node = self.node.name,
                    provider = record.provider.to_base58(),
                    record = bs58::encode(&record.key).into_string(),
                    "Received AddProvider"
                );
                self.print_providers(&record.key);
            }
            None => {
                warn!(node = self.node.name, "Received AddProvider with no record");
            }
        }
    }

    fn handle_inbound_request_get_provider(
        &mut self,
        _num_closer_peers: usize,
        _num_provider_peers: usize,
    ) {
        debug!(node = self.node.name, "Kad Received GetProvider")
    }
}

impl SwarmContext {
    pub(crate) fn put_record_into_vault(&mut self, record: kad::Record) {
        let dir = PathBuf::from("FILE_SHARE_SAVED_FILES").join(self.node.name.clone());
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(liberum_core::file_id_to_str(record.key.clone()));
        if let Err(e) = std::fs::write(path.clone(), record.value) {
            error!(
                node = self.node.name,
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
            node = self.node.name,
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
                node = self.node.name,
                provider = p.provider.to_base58(),
                "Provider"
            );
        }
    }
}
