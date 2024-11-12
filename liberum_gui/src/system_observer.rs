use liberum_core::{node_config::NodeConfig, types::NodeInfo};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Result};
use liberum_core::{DaemonRequest, DaemonResponse, DaemonResult};
use tokio::sync::mpsc::{Receiver, Sender};

use std::sync::Mutex;
use tracing::{debug, error};

#[derive(Default, Clone)]
pub struct SystemState {
    pub node_infos: Vec<NodeInfo>,
    pub node_configs: HashMap<String, NodeConfig>,
}

pub struct SystemObserver {
    rt: tokio::runtime::Runtime,
    pub system_state: Arc<Mutex<Option<SystemState>>>,
    to_daemon_sender: Option<Sender<DaemonRequest>>,
    from_daemon_receiver: Option<Receiver<DaemonResult>>,
    observed_node_configs: Arc<Mutex<HashSet<String>>>,
}

impl SystemObserver {
    pub fn new() -> Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;
        let path = Path::new("/tmp/liberum-core/");
        let contact =
            rt.block_on(async { liberum_core::connect(path.join("liberum-core-socket")).await });
        let (to_daemon_sender, from_daemon_receiver) = match contact {
            Ok(c) => c,
            Err(e) => {
                error!(
                    err = e.to_string(),
                    "Failed to connect to the core. Make sure the core is running!"
                );
                Err(anyhow!(e))?
            }
        };

        Ok(Self {
            rt,
            system_state: Arc::new(Mutex::new(None)),
            to_daemon_sender: Some(to_daemon_sender),
            from_daemon_receiver: Some(from_daemon_receiver),
            observed_node_configs: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn add_observed_config(&mut self, name: &str) {
        debug!(name = name.to_string(), "Adding observed config");
        self.observed_node_configs
            .lock()
            .unwrap()
            .insert(name.to_string());
    }

    pub fn remove_observed_config(&mut self, name: &str) {
        debug!(name = name.to_string(), "Removing observed config");
        self.observed_node_configs.lock().unwrap().remove(name);
    }

    pub fn run_update_loop(&mut self) -> tokio::task::JoinHandle<()> {
        debug!("Spawning update loop");

        let to_daemon_sender = self.to_daemon_sender.take().unwrap();
        let mut from_daemon_receiver = self.from_daemon_receiver.take().unwrap();
        let system_state = self.system_state.clone();
        let observed_configs = self.observed_node_configs.clone();

        let update_loop_handle = self.rt.spawn(async move {
            loop {
                debug!("Updating state");

                to_daemon_sender
                    .send(DaemonRequest::ListNodes)
                    .await
                    .expect("Failed to send message to the daemon");

                debug!("Send list nodes");

                let nodes = from_daemon_receiver
                    .recv()
                    .await
                    .expect("No response from the daemon")
                    .expect("Daemon returned error");

                debug!("Got list nodes");

                let nodes = match nodes {
                    DaemonResponse::NodeList(list) => list,
                    _ => panic!("expected node list"),
                };

                let mut configs = HashMap::new();

                debug!("Getting observed configs");

                let config_node_names = observed_configs
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>();

                for config_node_name in config_node_names {
                    to_daemon_sender
                        .send(DaemonRequest::GetNodeConfig {
                            name: config_node_name.to_string(),
                        })
                        .await
                        .expect("Failed to send message to the daemon");

                    let node_config = from_daemon_receiver
                        .recv()
                        .await
                        .expect("No response from the daemon")
                        .expect("Daemon returned error");

                    let node_config = match node_config {
                        DaemonResponse::NodeConfig(config) => config,
                        _ => panic!("expected node config"),
                    };

                    configs.insert(config_node_name.to_string(), node_config);
                }

                debug!("Got observed configs");

                system_state.lock().unwrap().replace(SystemState {
                    node_infos: nodes,
                    node_configs: configs,
                });

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        debug!("Update loop spawned");

        update_loop_handle
    }
}
