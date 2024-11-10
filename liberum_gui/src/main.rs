use std::{path::Path, sync::Arc};

use anyhow::{anyhow, Result};
use egui::Color32;
use kameo::{
    actor::ActorRef, mailbox::bounded::BoundedMailbox, message::Message,
    request::BlockingMessageSend, Actor,
};
use liberum_core::{types::NodeInfo, DaemonRequest, DaemonResponse, DaemonResult};
use tokio::{
    sync::{
        mpsc::{Receiver, Sender},
        Mutex,
    },
    task::{spawn_blocking, JoinHandle},
};
use tracing::error;

#[derive(Default, Clone)]
struct SystemState {
    node_infos: Vec<NodeInfo>,
}

struct SystemObserver {
    system_state: Arc<Mutex<Option<SystemState>>>,
    updater_loop_handle: Option<JoinHandle<()>>,
    to_daemon_sender: Option<Sender<DaemonRequest>>,
    from_daemon_receiver: Option<Receiver<DaemonResult>>,
}

struct GetState;

impl SystemObserver {
    fn new(
        to_daemon_sender: Sender<DaemonRequest>,
        from_daemon_receiver: Receiver<DaemonResult>,
    ) -> Self {
        Self {
            system_state: Arc::new(Mutex::new(None)),
            updater_loop_handle: None,
            to_daemon_sender: Some(to_daemon_sender),
            from_daemon_receiver: Some(from_daemon_receiver),
        }
    }
}

impl Actor for SystemObserver {
    type Mailbox = BoundedMailbox<Self>;

    async fn on_start(
        &mut self,
        _: kameo::actor::ActorRef<Self>,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        let tds = self.to_daemon_sender.take().unwrap();
        let mut fdr = self.from_daemon_receiver.take().unwrap();
        let state = self.system_state.clone();

        self.updater_loop_handle = Some(tokio::spawn(async move {
            loop {
                tds.send(DaemonRequest::ListNodes)
                    .await
                    .expect("Failed to send message to the daemon");

                let nodes = fdr
                    .recv()
                    .await
                    .expect("No response from the daemon")
                    .expect("Daemon returned error");

                let nodes = match nodes {
                    DaemonResponse::NodeList(list) => list,
                    _ => panic!("expected node list"),
                };

                state
                    .lock()
                    .await
                    .replace(SystemState { node_infos: nodes });
            }
        }));

        Ok(())
    }

    async fn on_stop(
        self,
        _: kameo::actor::WeakActorRef<Self>,
        _: kameo::error::ActorStopReason,
    ) -> std::result::Result<(), kameo::error::BoxError> {
        if let Some(handle) = self.updater_loop_handle {
            handle.abort();
        }

        Ok(())
    }
}

impl Message<GetState> for SystemObserver {
    type Reply = Option<SystemState>;

    async fn handle(
        &mut self,
        _: GetState,
        _: kameo::message::Context<'_, Self, Self::Reply>,
    ) -> Self::Reply {
        self.system_state.lock().await.clone()
    }
}

struct MyApp {
    observer: ActorRef<SystemObserver>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let path = Path::new("/tmp/liberum-core/");
    let contact = liberum_core::connect(path.join("liberum-core-socket")).await;
    let (request_sender, response_receiver) = match contact {
        Ok(c) => c,
        Err(e) => {
            error!(
                err = e.to_string(),
                "Failed to connect to the core. Make sure the core is running!"
            );
            Err(anyhow!(e))?
        }
    };

    let system_observer = SystemObserver::new(request_sender, response_receiver);
    let system_observer = kameo::spawn(system_observer);
    let my_app = MyApp::new(system_observer)?;

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

impl MyApp {
    fn new(observer: ActorRef<SystemObserver>) -> Result<Self> {
        Ok(Self { observer })
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        spawn_blocking(|| {
            let system_state = self.observer.ask(GetState).blocking_send().unwrap();

            egui::CentralPanel::default().show(ctx, |ui| {
                let system_state = match system_state {
                    Some(state) => state,
                    None => {
                        ui.heading("No system state available");
                        return;
                    }
                };

                ui.heading("Nodes list:");
                system_state.node_infos.iter().for_each(|n| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.colored_label(
                                Color32::from_rgb(0, 100, 200),
                                format!("Node: {}", n.name),
                            );
                            ui.label(format!("Is running: {}", n.is_running));
                            ui.add_space(10.0);
                        });
                        ui.vertical(|ui| if ui.button("Run").clicked() {});
                    });
                })
            });
        });
    }
}
