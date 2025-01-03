use egui::Color32;

use crate::views::{NodeView, ViewAction};

use super::Window;

pub struct NodeListWindow {
    state: NodeListWindowState,
}

#[derive(Clone)]
pub struct NodeListWindowState {
    pub create_node_name: String,
    pub is_opened: bool,
}

pub struct NodeListWindowUpdate {
    pub view_action: ViewAction,
}

impl NodeListWindow {
    pub fn new() -> Self {
        Self {
            state: NodeListWindowState {
                create_node_name: String::new(),
                is_opened: false,
            },
        }
    }
}

impl Window<NodeListWindowState, NodeListWindowUpdate> for NodeListWindow {
    fn from_state(state: NodeListWindowState) -> Self {
        Self { state }
    }

    fn get_state(&self) -> NodeListWindowState {
        self.state.clone()
    }

    fn draw(&mut self, view_ctx: &mut crate::views::ViewContext) -> NodeListWindowUpdate {
        let state = view_ctx.system_state.lock().unwrap();
        let state = (*state).clone();
        let mut node_list_window_update = NodeListWindowUpdate {
            view_action: ViewAction::Stay,
        };

        egui::Window::new("Nodes")
            .default_pos([32.0, 64.0])
            .show(view_ctx.egui_ctx, |ui| {
                let state = match state {
                    Some(s) => s,
                    None => {
                        ui.heading("No system state available");
                        return;
                    }
                };

                ui.label("Create a new node");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.state.create_node_name);

                    if ui.button("Create").clicked() {
                        view_ctx
                            .daemon_com
                            .create_node(&mut self.state.create_node_name)
                            .unwrap();

                        self.state.create_node_name = String::new();
                    }
                });

                ui.add_space(20.0);

                egui::Grid::new("config_grid")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        state.node_infos.iter().for_each(|n| {
                            ui.colored_label(
                                Color32::from_rgb(0, 100, 200),
                                format!("Node: {}", n.name),
                            );

                            ui.label(format!("Is running: {}", n.is_running));

                            ui.horizontal(|ui| {
                                if ui.button("Run").clicked() {
                                    let _ = view_ctx.daemon_com.run_node(&n.name);
                                }

                                if ui.button("Stop").clicked() {
                                    let _ = view_ctx.daemon_com.stop_node(&n.name);
                                }

                                if ui.button("Show").clicked() {
                                    node_list_window_update.view_action = ViewAction::SwitchView {
                                        view: Box::new(NodeView::new(&n.name)),
                                    };
                                }
                            });

                            ui.end_row();
                        });
                    });
            });

        node_list_window_update
    }

    fn is_opened(&self) -> bool {
        self.state.is_opened
    }

    fn open(&mut self) {
        self.state.is_opened = true;
    }

    fn close(&mut self) {
        self.state.is_opened = false;
    }
}
