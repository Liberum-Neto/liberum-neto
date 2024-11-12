use std::path::{Path, PathBuf};

use egui::Color32;
use egui_file::FileDialog;
use liberum_core::types::NodeInfo;

use super::{AppView, NodesListView, ViewAction, ViewContext};

pub struct NodeView {
    node_name: String,
    file_to_send_path: Option<PathBuf>,
    file_to_send_dialog: Option<FileDialog>,
    file_to_download_id: String,
    config_window_opened: bool,
    status_line: String,
}

impl NodeView {
    pub fn new(node_name: &str) -> Self {
        Self {
            node_name: node_name.to_string(),
            file_to_send_path: None,
            file_to_send_dialog: None,
            file_to_download_id: String::new(),
            config_window_opened: false,
            status_line: String::new(),
        }
    }

    fn show_default_panel(&mut self, ctx: &mut ViewContext) {
        egui::CentralPanel::default().show(ctx.egui_ctx, |ui| {
            ui.heading("Node details page");
        });
    }

    fn show_node_window(&mut self, ctx: &mut ViewContext) {
        egui::Window::new("Node")
            .default_pos([32.0, 64.0])
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

                let node_infos = system_state
                    .node_infos
                    .into_iter()
                    .filter(|n| n.name == self.node_name)
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
                        for addr in &node_info.addresses {
                            ui.label(addr);
                        }
                    });

                    if node_info.addresses.is_empty() {
                        ui.label("No addresses");
                    }
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Run").clicked() {
                        let _ = ctx.event_handler.run_node(&node_info.name);
                    }

                    if ui.button("Stop").clicked() {
                        let _ = ctx.event_handler.stop_node(&node_info.name);
                    }

                    if ui.button("Config").clicked() {
                        self.config_window_opened = true;
                    }
                });

                ui.add_space(20.0);
                ui.heading("Send files");

                let file_selected_text = self
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
                        let mut dialog = FileDialog::open_file(self.file_to_send_path.clone())
                            .show_files_filter(filter);
                        dialog.open();
                        self.file_to_send_dialog = Some(dialog);
                    }

                    if let Some(dialog) = &mut self.file_to_send_dialog {
                        if dialog.show(ctx.egui_ctx).selected() {
                            if let Some(file_path) = dialog.path() {
                                self.file_to_send_path = Some(file_path.to_path_buf());
                            }
                        }
                    }

                    if ui.button("Publish file").clicked() {
                        match &self.file_to_send_path {
                            Some(path) => {
                                let result = ctx.event_handler.publish_file(&self.node_name, &path);
                                match result {
                                    Ok(_) => self.status_line = "File published!".to_string(),
                                    Err(e) => self.status_line = e.to_string(),
                                };
                            }
                            None => {
                                self.status_line = "Error: No file selected".to_string();
                            }
                        }
                    }
                });

                ui.add_space(20.0);

                ui.heading("Download file");
                ui.label("File ID:");
                ui.text_edit_singleline(&mut self.file_to_download_id);
                ui.add_space(10.0);

                if ui.button("Download").clicked() {
                    match ctx
                        .event_handler
                        .download_file(&self.node_name, &self.file_to_download_id)
                    {
                        Ok(_) => {
                            self.status_line = "File downloaded".to_string();
                            self.file_to_download_id = String::new();
                        }
                        Err(e) => self.status_line = e.to_string(),
                    }
                }

                ui.add_space(20.0);
            });
    }

    fn show_config_window(&mut self, ctx: &mut ViewContext) {
        egui::Window::new("Configuration")
            .open(&mut self.config_window_opened)
            .show(ctx.egui_ctx, |ui| {
                ui.label("some config options here");
            });
    }

    fn show_status_bar(&mut self, ctx: &mut ViewContext) -> ViewAction {
        let mut action = ViewAction::Stay;

        egui::TopBottomPanel::bottom(egui::Id::new("status_bar"))
            .frame(egui::Frame::default().inner_margin(16.0))
            .show_separator_line(false)
            .show(ctx.egui_ctx, |ui| {
                ui.label(&self.status_line);

                if ui.button("Back to nodes list").clicked() {
                    action = ViewAction::SwitchView {
                        view: Box::new(NodesListView::default()),
                    }
                }
            });

        action
    }
}

impl AppView for NodeView {
    fn draw(&mut self, mut ctx: ViewContext) -> ViewAction {
        self.show_default_panel(&mut ctx);
        self.show_config_window(&mut ctx);
        self.show_node_window(&mut ctx);
        self.show_status_bar(&mut ctx)
    }
}