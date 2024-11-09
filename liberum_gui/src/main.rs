use std::path::Path;

use anyhow::{anyhow, Result};
use liberum_core::{DaemonRequest, DaemonResponse, DaemonResult};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::error;

fn main() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    let my_app = MyApp::new()?;

    eframe::run_native(
        "liberum-gui",
        options,
        Box::new(|_| Ok(Box::<MyApp>::new(my_app))),
    )
    .map_err(|e| anyhow!(e.to_string()))?;

    Ok(())
}

struct MyApp {
    rt: tokio::runtime::Runtime,
    to_daemon_sender: Sender<DaemonRequest>,
    from_daemon_receiver: Receiver<DaemonResult>,
}

impl MyApp {
    fn new() -> Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let path = Path::new("/tmp/liberum-core/");
        let contact =
            rt.block_on(async { liberum_core::connect(path.join("liberum-core-socket")).await });

        let (request_sender, response_receiver) = match contact {
            Ok(c) => c,
            Err(e) => {
                error!(
                    err = e.to_string(),
                    "Failed to connect to the core. Make sure the client is running!"
                );
                Err(anyhow!(e))?
            }
        };

        Ok(Self {
            rt,
            to_daemon_sender: request_sender,
            from_daemon_receiver: response_receiver,
        })
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Nodes list:");

            self.rt.block_on(async {
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

                ui.vertical(|ui| {
                    nodes.iter().for_each(|n| {
                        ui.label(format!("Node: {}", n.name));
                    });
                });
            });
        });
    }
}
