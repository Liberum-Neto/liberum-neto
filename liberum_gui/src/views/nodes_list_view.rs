use crate::windows::{node_list_window::NodeListWindow, Window};

use super::{AppView, ViewAction, ViewContext};

pub struct NodesListView {
    node_list_window: NodeListWindow,
}

impl NodesListView {
    pub fn new() -> Self {
        Self {
            node_list_window: NodeListWindow::new(),
        }
    }
}

impl AppView for NodesListView {
    fn draw(&mut self, ctx: &mut ViewContext) -> ViewAction {
        let mut update = None;

        egui::CentralPanel::default().show(ctx.egui_ctx, |ui| {
            ui.heading("Nodes list page");
            update = Some(self.node_list_window.draw(ctx));
        });

        update.unwrap().view_action
    }
}
