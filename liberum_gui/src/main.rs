pub mod daemon_com;
pub mod system_observer;
pub mod views;

use std::{cell::RefCell, rc::Rc, sync::Arc};

use anyhow::{anyhow, Result};
use daemon_com::DaemonCom;
use egui::Visuals;
use system_observer::{SystemObserver, SystemState};
use views::{AppView, NodesListView, ViewAction, ViewContext};

use std::sync::Mutex;
use tracing::debug;

struct MyApp {
    current_view: Box<dyn AppView>,
    system_state: Arc<Mutex<Option<SystemState>>>,
    system_observer: Rc<RefCell<SystemObserver>>,
    daemon_com: DaemonCom,
}

impl MyApp {
    fn new(system_observer: Rc<RefCell<SystemObserver>>, daemon_com: DaemonCom) -> Self {
        Self {
            current_view: Box::new(NodesListView::default()),
            system_state: system_observer.borrow().system_state.clone(),
            system_observer: system_observer.clone(),
            daemon_com,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.style_mut(|style| {
            style.visuals = Visuals::light();
        });

        let mut view_ctx = ViewContext {
            system_state: self.system_state.clone(),
            daemon_com: &mut self.daemon_com,
            system_observer: self.system_observer.clone(),
            egui_ctx: ctx,
            _egui_frame: frame,
        };

        let action = self.current_view.draw(&mut view_ctx);

        match action {
            ViewAction::Stay => {}
            ViewAction::SwitchView { view } => {
                self.current_view.teardown(&mut view_ctx);
                self.current_view = view;
                self.current_view.setup(&mut view_ctx);
            }
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let system_observer = Rc::new(RefCell::new(SystemObserver::new()?));
    let daemon_com = DaemonCom::new()?;
    let my_app = MyApp::new(system_observer.clone(), daemon_com);

    debug!("Running observer loop");
    let update_loop_handle = system_observer.borrow_mut().run_update_loop();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };

    debug!("Opening window");
    eframe::run_native(
        "liberum-gui",
        options,
        Box::new(|_| Ok(Box::<MyApp>::new(my_app))),
    )
    .map_err(|e| anyhow!(e.to_string()))
    .unwrap();

    update_loop_handle.abort();

    Ok(())
}
