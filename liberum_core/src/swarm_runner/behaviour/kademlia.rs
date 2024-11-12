use std::collections::HashSet;

use anyhow::anyhow;
use libp2p::kad::{self, store::RecordStore};
use tokio::sync::oneshot;
use tracing::{debug, info};

use crate::swarm_runner::SwarmContext;

/// Methods on SwarmContext for handling Kademlia
/// On QueryProgressed events generally it is required to remember the query ID
/// from when the query was started to react to the event
impl SwarmContext {
    pub(crate) fn handle_kademlia(&mut self, event: kad::Event) {
        match event {
            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::StartProviding(result),
                ..
            } => {
                // result is matched two times to trace the result no matter
                // if the response is sent via the oneshot channel or not
                if result.is_ok() {
                    info!(
                        node = self.node.name,
                        id = format!("{id:?}"),
                        "Started providing file"
                    );
                } else {
                    debug!(
                        node = self.node.name,
                        id = format!("{id:?}"),
                        "Failed to start providing file"
                    );
                }

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

            kad::Event::OutboundQueryProgressed {
                id,
                result: kad::QueryResult::PutRecord(result),
                ..
            } => {
                if result.is_ok() {
                    info!(node = self.node.name, id = format!("{id:?}"), "Put file");
                } else {
                    debug!(
                        node = self.node.name,
                        id = format!("{id:?}"),
                        "Failed to put file"
                    );
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
                    debug!(qid = format!("{id}"), "Channel closed");
                }
            }

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
                            qid = format!("{id}"),
                            err = format!("{e:?}"),
                            "Channel closed"
                        )
                    });
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .query_mut(&id)
                        .unwrap()
                        .finish();
                }
            }
            kad::Event::OutboundQueryProgressed {
                id,
                result:
                    kad::QueryResult::GetProviders(Ok(
                        kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. },
                    )),
                ..
            } => {
                debug!("Get providers didn't find any new records");
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
            kad::Event::InboundRequest {
                request:
                    kad::InboundRequest::PutRecord {
                        source,
                        connection,
                        record,
                    },
            } => 'break_inbound_put_record: {
                if record.is_none() {
                    debug!("Received PutRecord with no record");
                    return;
                }
                let record = record.unwrap();

                debug!(node = self.node.name, "Received PutRecord");

                // To check if announcing providing is successfull we would need to wait for another event,
                // but that would block the event loop, so we need to assume it is successful
                // The record is being provided for sure, we just don't know if the information was
                // published properly
                let provide = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(record.key.clone());
                if provide.is_err() {
                    debug!(
                        node = self.node.name,
                        err = format!("{provide:?}"),
                        "Failed to start providing file"
                    );
                    break 'break_inbound_put_record;
                }
                let provide_qid = provide.unwrap();

                let (sender, receiver) = oneshot::channel();
                self.behaviour
                    .pending_start_providing
                    .insert(provide_qid, sender);

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
                        "Failed to put record"
                    );
                    break 'break_inbound_put_record;
                }

                let r = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(record.key.clone());
                if r.is_err() {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .store_mut()
                        .remove(&record.key);
                    debug!(
                        node = self.node.name,
                        err = format!("{r:?}"),
                        "Failed to start providing file"
                    );
                    break 'break_inbound_put_record;
                }
            }
            _ => {}
        }
    }
}
