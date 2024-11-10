use std::collections::HashSet;

use libp2p::kad;
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
                result: kad::QueryResult::StartProviding(_),
                ..
            } => {
                info!(
                    node = self.node.name,
                    id = format!("{id:?}"),
                    "Published file"
                );
                let sender: oneshot::Sender<()> = self
                    .behaviour
                    .pending_start_providing
                    .remove(&id)
                    .expect("Query ID to not disappear from hashmap.");

                // Node is waiting on its oneshot for this message to know
                // that the file was published
                let _ = sender.send(());
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
                    sender.send(providers).expect("Channel not to break");
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
                    sender.send(HashSet::new()).expect("Channel not to break");
                    //self.swarm.behaviour_mut().kademlia.query_mut(&id).unwrap().finish();
                }
            }
            kad::Event::InboundRequest {
                request: kad::InboundRequest::GetProvider { .. },
            } => {
                debug!(node = self.node.name, "Received GetProvider")
            }
            _ => {}
        }
    }
}
