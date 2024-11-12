pub mod node_view;
pub mod nodes_list_view;

pub use node_view::NodeView;
pub use nodes_list_view::NodesListView;

use std::sync::Arc;

use std::sync::Mutex;

use crate::{event_handler::EventHandler, system_observer::SystemState};

pub trait AppView {
    fn draw(&mut self, ctx: ViewContext) -> ViewAction;
}

pub enum ViewAction {
    Stay,
    SwitchView { view: Box<dyn AppView> },
}

pub struct ViewContext<'a> {
    pub system_state: Arc<Mutex<Option<SystemState>>>,
    pub event_handler: &'a mut EventHandler,
    pub egui_ctx: &'a egui::Context,
    pub _egui_frame: &'a mut eframe::Frame,
}
