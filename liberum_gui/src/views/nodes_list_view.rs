use egui::Color32;

use super::{AppView, NodeView, ViewAction, ViewContext};

#[derive(Default)]
pub struct NodesListView {
    create_node_name: String,
}

impl AppView for NodesListView {
    fn draw(&mut self, ctx: ViewContext) -> ViewAction {
        let state = ctx.system_state.lock().unwrap();
        let state = (*state).clone();
        let mut action = ViewAction::Stay;

        egui::CentralPanel::default().show(ctx.egui_ctx, |ui| {
            let state = match state {
                Some(s) => s,
                None => {
                    ui.heading("No system state available");
                    return;
                }
            };

            ui.heading("Create a new node");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.create_node_name);

                if ui.button("Create").clicked() {
                    ctx.event_handler
                        .create_node(&mut self.create_node_name)
                        .unwrap();

                    self.create_node_name = String::new();
                }
            });

            ui.add_space(10.0);

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

                    if ui.button("Run").clicked() {
                        let _ = ctx.event_handler.run_node(&n.name);
                    }

                    if ui.button("Stop").clicked() {
                        let _ = ctx.event_handler.stop_node(&n.name);
                    }

                    if ui.button("Show").clicked() {
                        action = ViewAction::SwitchView {
                            view: Box::new(NodeView::new(&n.name)),
                        };
                    }
                });
            });

            ui.add_space(10.0);
        });

        action
    }
}
