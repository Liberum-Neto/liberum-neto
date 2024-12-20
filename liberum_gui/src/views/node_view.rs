use std::path::{Path, PathBuf};

use egui::{Align2, Color32};
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
    download_window_opened: bool,
    download_data: Vec<u8>,
    dial_peer_id: String,
    dial_addr: String,
    dial_history: Vec<(String, String, bool)>,
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
            download_window_opened: false,
            download_data: Vec::new(),
            dial_peer_id: String::new(),
            dial_addr: String::new(),
            dial_history: Vec::new(),
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
                        let _ = ctx.daemon_com.run_node(&node_info.name);
                    }

                    if ui.button("Stop").clicked() {
                        let _ = ctx.daemon_com.stop_node(&node_info.name);
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
                                let result = ctx.daemon_com.publish_file(&self.node_name, &path);
                                match result {
                                    Ok(id) => self.status_line = format!("File published; id={id}"),
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
                        .daemon_com
                        .download_file(&self.node_name, &self.file_to_download_id)
                    {
                        Ok(data) => {
                            self.status_line = "File downloaded".to_string();
                            self.file_to_download_id = String::new();
                            self.download_window_opened = true;
                            self.download_data = data;
                        }
                        Err(e) => self.status_line = e.to_string(),
                    }
                }

                ui.add_space(20.0);
            });
    }

    fn show_dialer_window(&mut self, ctx: &mut ViewContext) {
        egui::Window::new("Dialer")
            .anchor(Align2::RIGHT_TOP, [-16.0, 16.0])
            .show(ctx.egui_ctx, |ui| {
                egui::TopBottomPanel::top("dial_controls").show_inside(ui, |ui| {
                    ui.label("PeerID:");
                    ui.text_edit_singleline(&mut self.dial_peer_id);
                    ui.label("Peer address:");
                    ui.text_edit_singleline(&mut self.dial_addr);

                    ui.add_space(10.0);

                    if ui.button("Dial").clicked() {
                        match ctx.daemon_com.dial(
                            &self.node_name,
                            &self.dial_peer_id,
                            &self.dial_addr,
                        ) {
                            Ok(_) => {
                                self.status_line = format!(
                                    "Dial {} @ {} successful!",
                                    self.dial_peer_id, self.dial_addr
                                );

                                self.dial_history.push((
                                    self.dial_peer_id.clone(),
                                    self.dial_addr.clone(),
                                    true,
                                ));

                                self.dial_peer_id = String::new();
                                self.dial_addr = String::new();
                            }
                            Err(e) => {
                                self.status_line = e.to_string();

                                self.dial_history.push((
                                    self.dial_peer_id.clone(),
                                    self.dial_addr.clone(),
                                    false,
                                ));
                            }
                        }
                    }

                    ui.add_space(10.0);

                    if !self.dial_history.is_empty() {
                        egui::Grid::new("dial_history")
                            .num_columns(3)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("Peer ID");
                                ui.label("Peer address");
                                ui.label("Successful?");
                                ui.end_row();

                                for (peer_id, peer_addr, success) in &self.dial_history {
                                    ui.label(peer_id);
                                    ui.label(peer_addr);
                                    ui.label(success.to_string());
                                    ui.end_row();
                                }
                            });
                    }
                });
            });
    }

    fn show_config_window(&mut self, ctx: &mut ViewContext) {
        egui::Window::new("Configuration")
            .open(&mut self.config_window_opened)
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

                let node_config = system_state.node_configs.get(&self.node_name);

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
    }

    fn show_download_window(&mut self, ctx: &mut ViewContext) {
        egui::Window::new("Download info")
            .open(&mut self.download_window_opened)
            .show(ctx.egui_ctx, |ui| {
                ui.label(String::from_utf8(self.download_data.clone()).unwrap());
            });
    }

    fn show_status_bar(&mut self, ctx: &mut ViewContext) -> ViewAction {
        let mut action = ViewAction::Stay;

        egui::TopBottomPanel::bottom(egui::Id::new("status_bar"))
            .frame(egui::Frame::default().inner_margin(16.0))
            .show_separator_line(false)
            .show(ctx.egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Back to nodes list").clicked() {
                        action = ViewAction::SwitchView {
                            view: Box::new(NodesListView::default()),
                        }
                    }

                    ui.label(&self.status_line);
                });
            });

        action
    }
}

impl AppView for NodeView {
    fn setup(&mut self, ctx: &mut ViewContext) {
        ctx.system_observer
            .borrow_mut()
            .add_observed_config(&self.node_name);
    }

    fn draw(&mut self, mut ctx: &mut ViewContext) -> ViewAction {
        self.show_default_panel(&mut ctx);
        self.show_config_window(&mut ctx);
        self.show_node_window(&mut ctx);
        self.show_download_window(&mut ctx);
        self.show_dialer_window(&mut ctx);
        self.show_status_bar(&mut ctx)
    }

    fn teardown(&mut self, ctx: &mut ViewContext) {
        ctx.system_observer
            .borrow_mut()
            .remove_observed_config(&self.node_name);
    }
}
