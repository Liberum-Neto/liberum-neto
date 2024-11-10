use std::{path::Path, sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use egui::Color32;
use liberum_core::{types::NodeInfo, DaemonRequest, DaemonResponse, DaemonResult};
use tokio::sync::mpsc::{Receiver, Sender};

use std::sync::Mutex;
use tracing::{debug, error, info};

#[derive(Default, Clone)]
struct SystemState {
    node_infos: Vec<NodeInfo>,
}

struct SystemObserver {
    rt: tokio::runtime::Runtime,
    system_state: Arc<Mutex<Option<SystemState>>>,
    to_daemon_sender: Sender<DaemonRequest>,
    from_daemon_receiver: Receiver<DaemonResult>,
}

struct EventHandler {
    rt: tokio::runtime::Runtime,
    to_daemon_sender: Sender<DaemonRequest>,
    from_daemon_receiver: Receiver<DaemonResult>,
}

impl EventHandler {
    fn new() -> Result<Self> {
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

    fn run_node(&mut self, name: &str) -> Result<()> {
        debug!(name = name.to_string(), "Trying to run node");

        self.rt.block_on(async {
            self.to_daemon_sender
                .send(DaemonRequest::StartNode {
                    name: name.to_string(),
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
}

struct MyApp {
    system_state: Arc<Mutex<Option<SystemState>>>,
    event_handler: EventHandler,
}

impl SystemObserver {
    fn new() -> Result<Self> {
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
            system_state: Arc::new(Mutex::new(None)),
            to_daemon_sender,
            from_daemon_receiver,
        })
    }

    fn run_update_loop(mut self) -> tokio::task::JoinHandle<()> {
        debug!("Spawning update loop");

        let update_loop_handle = self.rt.spawn(async move {
            loop {
                debug!("Updating state");

                self.to_daemon_sender
                    .send(DaemonRequest::ListNodes)
                    .await
                    .expect("Failed to send message to the daemon");

                let nodes = self
                    .from_daemon_receiver
                    .recv()
                    .await
                    .expect("No response from the daemon")
                    .expect("Daemon returned error");

                let nodes = match nodes {
                    DaemonResponse::NodeList(list) => list,
                    _ => panic!("expected node list"),
                };

                self.system_state
                    .lock()
                    .unwrap()
                    .replace(SystemState { node_infos: nodes });

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        debug!("Update loop spawned");

        update_loop_handle
    }
}

impl MyApp {
    fn new(system_state: Arc<Mutex<Option<SystemState>>>, event_handler: EventHandler) -> Self {
        Self {
            system_state,
            event_handler,
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let system_observer = SystemObserver::new()?;
    let event_handler = EventHandler::new()?;
    let my_app = MyApp::new(system_observer.system_state.clone(), event_handler);

    debug!("Running observer loop...");
    system_observer.run_update_loop();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "liberum-gui",
        options,
        Box::new(|_| Ok(Box::<MyApp>::new(my_app))),
    )
    .map_err(|e| anyhow!(e.to_string()))
    .unwrap();

    Ok(())
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let state = self.system_state.lock().unwrap();
        let state = (*state).clone();

        egui::CentralPanel::default().show(ctx, |ui| {
            let state = match state {
                Some(s) => s,
                None => {
                    ui.heading("No system state available");
                    return;
                }
            };

            ui.heading("Nodes list:");
            state.node_infos.iter().for_each(|n| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.colored_label(
                            Color32::from_rgb(0, 100, 200),
                            format!("Node: {}", n.name),
                        );
                        ui.label(format!("Is running: {}", n.is_running));
                        ui.add_space(10.0);
                    });
                    ui.vertical(|ui| {
                        if ui.button("Run").clicked() {
                            let _ = self.event_handler.run_node(&n.name);
                        }
                    });
                });
            })
        });
    }
}
