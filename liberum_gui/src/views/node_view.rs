use std::{fs, path::PathBuf};

use anyhow::Result;
use egui::{Align2, Color32, RichText};
use egui_file::FileDialog;

use crate::windows::{
    dialer_window::DialerWindow, node_config_window::NodeConfigWindow, node_window::NodeWindow,
    Window,
};

use super::{AppView, NodesListView, ViewAction, ViewContext};

#[derive(Clone)]
struct FileInfo {
    id: String,
    path: PathBuf,
    size: usize,
    pins: Vec<String>,
}

pub struct NodeView {
    node_name: String,
    file_to_download_id: String,
    config_window: NodeConfigWindow,
    node_window: NodeWindow,
    dialer_window: DialerWindow,
    status_line: String,
    download_window_opened: bool,
    downloaded_file_info: Option<FileInfo>,
    download_destination_dialog: Option<FileDialog>,
    download_destination_path: Option<PathBuf>,
    download_history: Vec<Result<FileInfo>>,
    download_details_file_info: Option<FileInfo>,
}

impl NodeView {
    pub fn new(node_name: &str) -> Self {
        Self {
            node_name: node_name.to_string(),
            file_to_download_id: String::new(),
            config_window: NodeConfigWindow::new(node_name),
            node_window: NodeWindow::new(node_name),
            dialer_window: DialerWindow::new(node_name),
            status_line: String::new(),
            download_window_opened: false,
            downloaded_file_info: None,
            download_destination_dialog: None,
            download_destination_path: None,
            download_history: Vec::new(),
            download_details_file_info: None,
        }
    }

    fn show_default_panel(&mut self, ctx: &mut ViewContext) {
        egui::CentralPanel::default().show(ctx.egui_ctx, |ui| {
            ui.heading("Node details page");
        });
    }

    fn show_node_window(&mut self, ctx: &mut ViewContext) {
        let update = self.node_window.draw(ctx);

        if update.config_button_clicked {
            self.config_window.open();
        }

        if let Some(new_status_line) = update.new_status_line {
            self.status_line = new_status_line;
        }
    }

    fn show_dialer_window(&mut self, ctx: &mut ViewContext) {
        let update = self.dialer_window.draw(ctx);

        if let Some(new_status_line) = update.new_status_line {
            self.status_line = new_status_line;
        }
    }

    fn show_downloader_window(&mut self, ctx: &mut ViewContext) {
        egui::Window::new("Downloader")
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .show(ctx.egui_ctx, |ui| {
                egui::TopBottomPanel::top("downloader_controls").show_inside(ui, |ui| {
                    ui.heading("Download file");
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(0, 100, 200), "File ID:");
                        ui.text_edit_singleline(&mut self.file_to_download_id);
                    });
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(0, 100, 200), "Download destination:");
                        ui.label(
                            self.download_destination_path
                                .clone()
                                .map(|path| path.to_string_lossy().to_string())
                                .unwrap_or("Not selected".to_string()),
                        );
                    });
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button("Select destination").clicked() {
                            let mut dialog =
                                FileDialog::save_file(self.download_destination_path.clone());
                            dialog.open();
                            self.download_destination_dialog = Some(dialog);
                        }

                        if let Some(dialog) = &mut self.download_destination_dialog {
                            if dialog.show(ctx.egui_ctx).selected() {
                                if let Some(file_path) = dialog.path() {
                                    self.download_destination_path = Some(file_path.to_path_buf());
                                }
                            }
                        }

                        ui.add_space(10.0);

                        if ui.button("Download").clicked() {
                            match &self.download_destination_path {
                                Some(dest_path) => match self.file_to_download_id.is_empty() {
                                    true => {
                                        self.status_line =
                                            "File ID to download must not be empty!".to_string();
                                    }
                                    false => {
                                        match ctx.daemon_com.download_file(
                                            &self.node_name,
                                            &self.file_to_download_id,
                                        ) {
                                            Ok(data) => {
                                                let downloaded_file_info = FileInfo {
                                                    id: self.file_to_download_id.clone(),
                                                    path: self
                                                        .download_destination_path
                                                        .clone()
                                                        .unwrap(),
                                                    size: data.len(),
                                                    pins: vec![
                                                        bs58::encode(
                                                            "hello---------------------------",
                                                        )
                                                        .into_string(),
                                                        bs58::encode(
                                                            "p2p-----------------------------",
                                                        )
                                                        .into_string(),
                                                        bs58::encode(
                                                            "world---------------------------",
                                                        )
                                                        .into_string(),
                                                    ],
                                                };

                                                self.status_line = "File downloaded".to_string();
                                                self.downloaded_file_info =
                                                    Some(downloaded_file_info.clone());
                                                self.download_details_file_info =
                                                    Some(downloaded_file_info.clone());
                                                self.download_window_opened = true;
                                                self.download_history
                                                    .push(Ok(downloaded_file_info));

                                                match fs::write(dest_path.clone(), data) {
                                                    Ok(_) => {
                                                        self.status_line = format!("File saved!")
                                                    }
                                                    Err(e) => {
                                                        self.status_line =
                                                            format!("File saving failed, err={e}")
                                                    }
                                                }
                                            }
                                            Err(e) => self.status_line = e.to_string(),
                                        }
                                    }
                                },
                                None => {
                                    self.status_line =
                                        "Download destination is not selected".to_string();
                                }
                            }

                            self.file_to_download_id = String::new();
                            self.download_destination_path = None;
                        }
                    });

                    ui.add_space(10.0);
                });

                if !self.download_history.is_empty() {
                    egui::Grid::new("download_history")
                        .num_columns(3)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Object ID");
                            ui.label("Save path");
                            ui.label("Size");
                            ui.label("Pins count");
                            ui.label("Successful?");
                            ui.label("Details");
                            ui.end_row();

                            for download in &self.download_history {
                                match download {
                                    Ok(d) => {
                                        ui.label(d.id.clone());
                                        ui.label(d.path.clone().display().to_string());
                                        ui.label(d.size.to_string());
                                        ui.label(d.pins.len().to_string());
                                        ui.label("true");

                                        if ui.button("Details").clicked() {
                                            self.download_details_file_info = Some(d.clone());
                                            self.download_window_opened = true;
                                        }
                                    }
                                    Err(_) => {
                                        ui.label("-");
                                        ui.label("-");
                                        ui.label("-");
                                        ui.label("-");
                                        ui.label("false");
                                    }
                                }

                                ui.end_row();
                            }
                        });
                }
            });
    }

    fn show_config_window(&mut self, ctx: &mut ViewContext) {
        self.config_window.draw(ctx);
    }

    fn show_download_window(&mut self, ctx: &mut ViewContext) {
        egui::Window::new("Download info")
            .open(&mut self.download_window_opened)
            .show(ctx.egui_ctx, |ui| {
                let download_details_file_info = match self.download_details_file_info.clone() {
                    Some(info) => info,
                    None => return,
                };

                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Object ID:");
                    ui.label(download_details_file_info.id.as_str());
                });
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Save path:");
                    ui.label(&download_details_file_info.path.display().to_string());
                });
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Object size:");
                    ui.label(&download_details_file_info.size.to_string());
                });
                ui.add_space(10.0);
                ui.colored_label(
                    Color32::from_rgb(0, 200, 100),
                    RichText::heading("Pinned objects:".into()),
                );

                if !download_details_file_info.pins.is_empty() {
                    egui::Grid::new("downloaded_object_pins")
                        .num_columns(1)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Pinned object ID");
                            ui.end_row();

                            for obj_id in &download_details_file_info.pins {
                                ui.label(obj_id.to_string());
                                ui.end_row();
                            }
                        });
                }
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
                            view: Box::new(NodesListView::new()),
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
        self.show_downloader_window(&mut ctx);
        self.show_status_bar(&mut ctx)
    }

    fn teardown(&mut self, ctx: &mut ViewContext) {
        ctx.system_observer
            .borrow_mut()
            .remove_observed_config(&self.node_name);
    }
}
