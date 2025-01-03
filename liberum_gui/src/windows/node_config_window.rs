use crate::views::ViewContext;

pub struct NodeConfigWindow {
    state: NodeConfigWindowState,
}

#[derive(Clone)]
pub struct NodeConfigWindowState {
    node_name: String,
    is_opened: bool,
}

impl super::Window<NodeConfigWindowState, NodeConfigWindowState> for NodeConfigWindow {
    fn new(state: NodeConfigWindowState) -> Self {
        Self { state }
    }

    fn draw(&mut self, ctx: &mut ViewContext) -> NodeConfigWindowState {
        egui::Window::new("Configuration")
            .open(&mut self.state.is_opened)
            .show(ctx.egui_ctx, |ui| {
                let system_state = ctx.system_state.lock().unwrap();
                let system_state = (*system_state).clone();
                let system_state = match system_state {
                    Some(s) => s,
                    None => {
                        ui.heading("Could not get system state");
                        return;
                    }
                };

                let node_config = system_state.node_configs.get(&self.state.node_name);

                match node_config {
                    Some(cfg) => {
                        egui::Grid::new("config_grid")
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("Bootstrap nodes");
                                ui.vertical(|ui| {
                                    for b in cfg.bootstrap_nodes.iter() {
                                        ui.label(format!("{} @ {}", b.id, b.addr));
                                    }

                                    let mut text = String::new();

                                    ui.horizontal(|ui| {
                                        let _ = ui.text_edit_singleline(&mut text);
                                        let _ = ui.button("Add new");
                                    });
                                });
                                ui.end_row();
                                ui.label("External addresses");
                                ui.vertical(|ui| {
                                    for a in cfg.external_addresses.iter() {
                                        ui.horizontal(|ui| {
                                            ui.label(a.to_string());
                                            let _ = ui.button("Remove");
                                        });
                                    }

                                    let mut text = String::new();

                                    ui.horizontal(|ui| {
                                        let _ = ui.text_edit_singleline(&mut text);
                                        let _ = ui.button("Add new");
                                    });
                                });
                            });
                    }
                    None => {
                        ui.label("Config not found");
                    }
                };
            });

        self.state.clone()
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
