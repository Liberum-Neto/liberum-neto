use std::{collections::HashSet, path::PathBuf};

use anyhow::anyhow;
use libp2p::kad::{self, store::RecordStore, RecordKey};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

use crate::swarm_runner::{file_share, SwarmContext};

/// Methods on SwarmContext for handling Kademlia
/// On QueryProgressed events generally it is required to remember the query ID
/// from when the query was started to react to the event
impl SwarmContext {
    pub(crate) fn handle_kademlia(&mut self, event: kad::Event) {
        match event {
            // #############################################################################################################
            // Triggered when a node starts providing an ID
            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::StartProviding(result),
                ..
            } => {
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

            // #############################################################################################################
            // Triggered when a record is put in the DHT
            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::PutRecord(result),
                ..
            } => {
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

            // #############################################################################################################
            // Triggered when some providers are found for a file ID
            kad::Event::OutboundQueryProgressed {
                id,
                result:
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                        providers,
                        ..
                    })),
                ..
            } => {
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

            // #############################################################################################################
            // Triggered when no providers are found for a file ID
            kad::Event::OutboundQueryProgressed {
                id,
                result:
                    kad::QueryResult::GetProviders(Ok(
                        kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. },
                    )),
                ..
            } => {
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
            kad::Event::InboundRequest {
                request: kad::InboundRequest::GetProvider { .. },
            } => {
                debug!(node = self.node.name, "Received GetProvider")
            }
            // kad::Event::InboundRequest {
            //     request:
            //         kad::InboundRequest::PutRecord {
            //             source,
            //             connection,
            //             record,
            //         },
            // } => 'break_inbound_put_record: {
            //     if record.is_none() {
            //         debug!("Received PutRecord with no record");
            //         return;
            //     }
            //     let record = record.unwrap();

            //     debug!(node = self.node.name, "Received PutRecord");

            //     // To check if announcing providing is successfull we would need to wait for another event,
            //     // but that would block the event loop, so we need to assume it is successful
            //     // The record is being provided for sure, we just don't know if the information was
            //     // published properly
            //     let provide = self
            //         .swarm
            //         .behaviour_mut()
            //         .kademlia
            //         .start_providing(record.key.clone());
            //     if provide.is_err() {
            //         debug!(
            //             node = self.node.name,
            //             err = format!("{provide:?}"),
            //             "Failed to start providing file"
            //         );
            //         break 'break_inbound_put_record;
            //     }
            //     let provide_qid = provide.unwrap();

            //     let (sender, receiver) = oneshot::channel();
            //     self.behaviour
            //         .pending_start_providing
            //         .insert(provide_qid, sender);

            //     let r = self
            //         .swarm
            //         .behaviour_mut()
            //         .kademlia
            //         .store_mut()
            //         .put(record.clone());
            //     if r.is_err() {
            //         debug!(
            //             node = self.node.name,
            //             err = format!("{r:?}"),
            //             "Failed to put record"
            //         );
            //         break 'break_inbound_put_record;
            //     }

            //     let r = self
            //         .swarm
            //         .behaviour_mut()
            //         .kademlia
            //         .start_providing(record.key.clone());
            //     if r.is_err() {
            //         self.swarm
            //             .behaviour_mut()
            //             .kademlia
            //             .store_mut()
            //             .remove(&record.key);
            //         debug!(
            //             node = self.node.name,
            //             err = format!("{r:?}"),
            //             "Failed to start providing file"
            //         );
            //         break 'break_inbound_put_record;
            //     }
            // }
            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))),
                ..
            } => {
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

            // #############################################################################################################
            // Triggered when no record was found
            kad::Event::OutboundQueryProgressed {
                id,
                result:
                    kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FinishedWithNoAdditionalRecord {
                        ..
                    })),
                ..
            } => {
                debug!("Didn't find record in DHT {:?}", id);
                // inform the caller that the file was not found
                if let Some(sender) = self.behaviour.pending_download_file_dht.remove(&id) {
                    let _ = sender.send(Vec::new());
                }
            }

            // #############################################################################################################
            // Triggered when the query to get a record from the DHT finishes
            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::GetRecord(result),
                ..
            } => {
                debug!(
                    "Failed to get record from DHT {:?} result: {:?}",
                    id, result
                );
                if let Some(sender) = self.behaviour.pending_download_file_dht.remove(&id) {
                    let _ = sender.send(Vec::new());
                }
            }

            //#############################################################################################################
            //## Debug Prints only from this point
            //#############################################################################################################
            kad::Event::InboundRequest {
                request: kad::InboundRequest::AddProvider { record },
            } => {
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
            kad::Event::InboundRequest {
                request: kad::InboundRequest::GetProvider { .. },
            } => {
                debug!(node = self.node.name, "Kad Received GetProvider")
            }
            _ => {}
        }
    }

    pub(crate) fn put_record_into_vault(&mut self, record: kad::Record) {
        let dir = PathBuf::from("FILE_SHARE_SAVED_FILES").join(self.node.name.clone());
        std::fs::create_dir(&dir).ok();
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
            file_share::SharedResource::File(file_share::FileResource { path: path.clone() }),
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
