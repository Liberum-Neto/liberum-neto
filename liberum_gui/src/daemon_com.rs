use std::path::Path;

use anyhow::{anyhow, bail, Result};
use liberum_core::{DaemonRequest, DaemonResponse, DaemonResult};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{debug, error, info};

pub struct DaemonCom {
    pub rt: tokio::runtime::Runtime,
    pub to_daemon_sender: Sender<DaemonRequest>,
    pub from_daemon_receiver: Receiver<DaemonResult>,
}

impl DaemonCom {
    pub fn new() -> Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
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
            to_daemon_sender,
            from_daemon_receiver,
        })
    }

    pub fn run_node(&mut self, name: &str) -> Result<()> {
        debug!(name = name.to_string(), "Trying to run node");

        self.rt.block_on(async {
            self.to_daemon_sender
                .send(DaemonRequest::StartNode {
                    node_name: name.to_string(),
                })
                .await?;

            match self.from_daemon_receiver.recv().await {
                Some(r) => info!(response = format!("{r:?}"), "Daemon responds: {:?}", r),
                None => {
                    error!("Failed to receive response");
                }
            }

            anyhow::Ok(())
        })?;

        Ok(())
    }

    pub fn stop_node(&mut self, name: &str) -> Result<()> {
        debug!(name = name.to_string(), "Trying to stop node");

        self.rt.block_on(async {
            self.to_daemon_sender
                .send(DaemonRequest::StopNode {
                    node_name: name.to_string(),
                })
                .await?;

            match self.from_daemon_receiver.recv().await {
                Some(r) => info!(response = format!("{r:?}"), "Daemon responds: {:?}", r),
                None => {
                    error!("Failed to receive response");
                }
            }

            anyhow::Ok(())
        })?;

        Ok(())
    }

    pub fn create_node(&mut self, name: &str) -> Result<()> {
        debug!(name = name.to_string(), "Trying to create node");

        self.rt.block_on(async {
            self.to_daemon_sender
                .send(DaemonRequest::NewNode {
                    node_name: name.to_string(),
                    id_seed: None,
                })
                .await?;

            match self.from_daemon_receiver.recv().await {
                Some(r) => info!(response = format!("{r:?}"), "Daemon responds: {:?}", r),
                None => {
                    error!("Failed to receive response");
                }
            }

            anyhow::Ok(())
        })?;

        Ok(())
    }

    pub fn publish_file(&mut self, node_name: &str, file_path: &Path) -> Result<String> {
        debug!(
            name = node_name.to_string(),
            path = file_path.display().to_string(),
            "Trying to publish file"
        );

        self.rt.block_on(async {
            self.to_daemon_sender
                .send(DaemonRequest::PublishFile {
                    node_name: node_name.to_string(),
                    path: file_path.to_path_buf(),
                })
                .await?;

            match self.from_daemon_receiver.recv().await {
                Some(r) => {
                    match r {
                        Ok(DaemonResponse::FilePublished { id }) => return Ok(id),
                        Err(e) => {
                            error!(err = e.to_string(), "Error ocurred while publishing file!");
                            bail!("Error occured while publishing file: {}", e.to_string());
                        }
                        _ => {
                            error!("Unexpected response type");
                            bail!("Unexpected response type");
                        }
                    };
                }
                None => {
                    error!("Failed to receive response");
                    bail!("Failed to receive response from the daemon");
                }
            }
        })
    }

    pub fn download_file(&mut self, node_name: &str, file_id: &str) -> Result<Vec<u8>> {
        self.rt.block_on(async {
            self.to_daemon_sender
                .send(DaemonRequest::DownloadFile {
                    node_name: node_name.to_string(),
                    id: file_id.to_string(),
                })
                .await?;

            match self.from_daemon_receiver.recv().await {
                Some(r) => {
                    match r {
                        Ok(DaemonResponse::FileDownloaded { data }) => return Ok(data),
                        Err(e) => {
                            error!(err = e.to_string(), "Error ocurred while downloading file!");
                            bail!("Error occured while publishing file: {}", e.to_string());
                        }
                        _ => {
                            error!("Unexpected response type");
                            bail!("Unexpected response type");
                        }
                    };
                }
                None => {
                    error!("Failed to receive response");
                    bail!("Failed to receive response from the daemon");
                }
            }
        })
    }

    pub fn dial(&mut self, node_name: &str, peer_id: &str, addr: &str) -> Result<()> {
        self.rt.block_on(async {
            self.to_daemon_sender
                .send(DaemonRequest::Dial {
                    node_name: node_name.to_string(),
                    peer_id: peer_id.to_string(),
                    addr: addr.to_string(),
                })
                .await?;

            match self.from_daemon_receiver.recv().await {
                Some(r) => {
                    match r {
                        Ok(DaemonResponse::Dialed) => {}
                        Err(e) => {
                            error!(err = e.to_string(), "Error ocurred while dialing peer!");
                            bail!("Error occured while publishing file: {}", e.to_string());
                        }
                        _ => {
                            error!("Unexpected response type");
                            bail!("Unexpected response type");
                        }
                    };
                }
                None => {
                    error!("Failed to receive response");
                    bail!("Failed to receive response from the daemon");
                }
            };

            Ok(())
        })
    }
}
