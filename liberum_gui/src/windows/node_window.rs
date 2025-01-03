use std::path::{Path, PathBuf};

use egui::Color32;
use egui_file::FileDialog;
use liberum_core::types::NodeInfo;

use super::Window;

pub struct NodeWindow {
    state: NodeWindowState,
    file_to_send_dialog: Option<FileDialog>,
}

#[derive(Clone)]
pub struct NodeWindowState {
    node_name: String,
    file_to_send_path: Option<PathBuf>,
    is_opened: bool,
}

pub struct NodeWindowUpdate {
    pub config_button_clicked: bool,
    pub new_status_line: Option<String>,
}

impl Default for NodeWindowUpdate {
    fn default() -> Self {
        Self {
            config_button_clicked: false,
            new_status_line: None,
        }
    }
}

impl NodeWindow {
    pub fn new(node_name: &str) -> Self {
        Self {
            state: NodeWindowState {
                node_name: node_name.to_string(),
                file_to_send_path: None,
                is_opened: false,
            },
            file_to_send_dialog: None,
        }
    }
}

impl Window<NodeWindowState, NodeWindowUpdate> for NodeWindow {
    fn from_state(state: NodeWindowState) -> Self {
        Self {
            state,
            file_to_send_dialog: None,
        }
    }

    fn get_state(&self) -> NodeWindowState {
        self.state.clone()
    }

    fn draw(&mut self, view_ctx: &mut crate::views::ViewContext) -> NodeWindowUpdate {
        let mut update = NodeWindowUpdate::default();

        egui::Window::new("Node")
            .default_pos([32.0, 64.0])
            .show(view_ctx.egui_ctx, |ui| {
                let system_state = view_ctx.system_state.lock().unwrap();
                let system_state = (*system_state).clone();
                let system_state = match system_state {
                    Some(s) => s,
                    None => {
                        ui.heading("Could not get system state");
                        return;
                    }
                };

                let node_infos = system_state
                    .node_infos
                    .into_iter()
                    .filter(|n| n.name == self.state.node_name)
                    .collect::<Vec<NodeInfo>>();

                let node_info = match node_infos.first() {
                    Some(n) => n,
                    None => {
                        ui.heading("No node info available");
                        ui.label("No such node found in the system");
                        return;
                    }
                };

                ui.heading(format!("Node {}", node_info.name));

                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Name:");
                    ui.label(&node_info.name);
                });

                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Is running:");
                    ui.label(&node_info.is_running.to_string());
                });

                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Addresses:");

                    ui.vertical(|ui| {
                        for addr in &node_info.config_addresses {
                            ui.label(addr);
                        }
                    });

                    if node_info.config_addresses.is_empty() {
                        ui.label("No addresses");
                    }
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Run").clicked() {
                        let _ = view_ctx.daemon_com.run_node(&node_info.name);
                    }

                    if ui.button("Stop").clicked() {
                        let _ = view_ctx.daemon_com.stop_node(&node_info.name);
                    }

                    if ui.button("Config").clicked() {
                        update.config_button_clicked = true;
                    }
                });

                ui.add_space(20.0);
                ui.heading("Send files");

                let file_selected_text = self
                    .state
                    .file_to_send_path
                    .as_ref()
                    .map(|path| path.to_str().unwrap_or("Unprintable path"))
                    .unwrap_or("No file selected");

                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "File selected:");
                    ui.label(file_selected_text);
                });

                ui.horizontal(|ui| {
                    if ui.button("Select file").clicked() {
                        let filter = Box::new(move |path: &Path| -> bool { path.is_file() });
                        let mut dialog =
                            FileDialog::open_file(self.state.file_to_send_path.clone())
                                .show_files_filter(filter);
                        dialog.open();
                        self.file_to_send_dialog = Some(dialog);
                    }

                    if let Some(dialog) = &mut self.file_to_send_dialog {
                        if dialog.show(view_ctx.egui_ctx).selected() {
                            if let Some(file_path) = dialog.path() {
                                self.state.file_to_send_path = Some(file_path.to_path_buf());
                            }
                        }
                    }

                    if ui.button("Publish file").clicked() {
                        match &self.state.file_to_send_path {
                            Some(path) => {
                                let result = view_ctx
                                    .daemon_com
                                    .publish_file(&self.state.node_name, &path);
                                match result {
                                    Ok(id) => {
                                        update.new_status_line =
                                            Some(format!("File published; id={id}"))
                                    }
                                    Err(e) => update.new_status_line = Some(e.to_string()),
                                };
                            }
                            None => {
                                update.new_status_line =
                                    Some("Error: No file selected".to_string());
                            }
                        }
                    }
                });

                ui.add_space(20.0);
            });

        update
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
