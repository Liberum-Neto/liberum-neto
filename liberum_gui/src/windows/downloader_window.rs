use std::path::PathBuf;

use egui::{Align2, Color32};
use egui_file::FileDialog;

use super::{FileInfo, Window};

pub struct DownloaderWindow {
    state: DownloaderWindowState,
    download_destination_dialog: Option<FileDialog>,
}

impl DownloaderWindow {
    pub fn new(node_name: &str) -> Self {
        Self {
            state: DownloaderWindowState {
                file_to_download_id: String::new(),
                download_destination_path: None,
                node_name: node_name.to_string(),
                last_downloaded_file_info: None,
                download_history: Vec::new(),
                is_opened: false,
            },
            download_destination_dialog: None,
        }
    }
}

#[derive(Clone)]
pub struct DownloaderWindowState {
    file_to_download_id: String,
    download_destination_path: Option<PathBuf>,
    node_name: String,
    last_downloaded_file_info: Option<FileInfo>,
    download_history: Vec<DownloadHistoryEntry>,
    is_opened: bool,
}

#[derive(Default)]
pub struct DownloaderWindowUpdate {
    pub new_status_line: Option<String>,
    pub file_downloaded: Option<FileInfo>,
    pub display_file_info: Option<FileInfo>,
}

#[derive(Clone)]
enum DownloadHistoryEntry {
    Success(FileInfo),
    Failure,
}

impl Window<DownloaderWindowState, DownloaderWindowUpdate> for DownloaderWindow {
    fn from_state(state: DownloaderWindowState) -> Self {
        Self {
            state,
            download_destination_dialog: None,
        }
    }

    fn get_state(&self) -> DownloaderWindowState {
        self.state.clone()
    }

    fn set_state(&mut self, state: DownloaderWindowState) {
        self.state = state;
    }

    fn draw(&mut self, view_ctx: &mut crate::views::ViewContext) -> DownloaderWindowUpdate {
        let mut update = DownloaderWindowUpdate::default();

        egui::Window::new("Downloader")
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .show(view_ctx.egui_ctx, |ui| {
                egui::TopBottomPanel::top("downloader_controls").show_inside(ui, |ui| {
                    ui.heading("Download file");
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(0, 100, 200), "File ID:");
                        ui.text_edit_singleline(&mut self.state.file_to_download_id);
                    });
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(0, 100, 200), "Download destination:");
                        ui.label(
                            self.state
                                .download_destination_path
                                .clone()
                                .map(|path| path.to_string_lossy().to_string())
                                .unwrap_or("Not selected".to_string()),
                        );
                    });
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button("Select destination").clicked() {
                            let mut dialog =
                                FileDialog::save_file(self.state.download_destination_path.clone());
                            dialog.open();
                            self.download_destination_dialog = Some(dialog);
                        }

                        if let Some(dialog) = &mut self.download_destination_dialog {
                            if dialog.show(view_ctx.egui_ctx).selected() {
                                if let Some(file_path) = dialog.path() {
                                    self.state.download_destination_path =
                                        Some(file_path.to_path_buf());
                                }
                            }
                        }

                        ui.add_space(10.0);

                        if ui.button("Download").clicked() {
                            match &self.state.download_destination_path {
                                Some(dest_path) => {
                                    match self.state.file_to_download_id.is_empty() {
                                        true => {
                                            update.new_status_line = Some(
                                                "File ID to download must not be empty!"
                                                    .to_string(),
                                            );
                                        }
                                        false => {
                                            match view_ctx.daemon_com.download_file(
                                                &self.state.node_name,
                                                &self.state.file_to_download_id,
                                            ) {
                                                Ok(info) => {
                                                    let downloaded_file_info = FileInfo {
                                                        id: self.state.file_to_download_id.clone(),
                                                        path: self
                                                            .state
                                                            .download_destination_path
                                                            .clone()
                                                            .unwrap(),
                                                        size: info.data.len(),
                                                        pins: info.pins,
                                                    };

                                                    update.new_status_line =
                                                        Some("File downloaded".to_string());
                                                    self.state.last_downloaded_file_info =
                                                        Some(downloaded_file_info.clone());
                                                    update.file_downloaded =
                                                        Some(downloaded_file_info.clone());
                                                    self.state.download_history.push(
                                                        DownloadHistoryEntry::Success(
                                                            downloaded_file_info,
                                                        ),
                                                    );

                                                    match std::fs::write(
                                                        dest_path.clone(),
                                                        info.data,
                                                    ) {
                                                        Ok(_) => {
                                                            update.new_status_line =
                                                                Some(format!("File saved!"))
                                                        }
                                                        Err(e) => {
                                                            update.new_status_line = Some(format!(
                                                                "File saving failed, err={e}"
                                                            ))
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    self.state
                                                        .download_history
                                                        .push(DownloadHistoryEntry::Failure);
                                                    update.new_status_line = Some(e.to_string())
                                                }
                                            }
                                        }
                                    }
                                }
                                None => {
                                    update.new_status_line =
                                        Some("Download destination is not selected".to_string());
                                }
                            }

                            self.state.file_to_download_id = String::new();
                            self.state.download_destination_path = None;
                        }
                    });

                    ui.add_space(10.0);
                });

                if !self.state.download_history.is_empty() {
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

                            for entry in &self.state.download_history {
                                match entry {
                                    DownloadHistoryEntry::Success(file_info) => {
                                        ui.label(file_info.id.clone());
                                        ui.label(file_info.path.clone().display().to_string());
                                        ui.label(file_info.size.to_string());
                                        ui.label(file_info.pins.len().to_string());
                                        ui.label("true");

                                        if ui.button("Details").clicked() {
                                            update.display_file_info = Some(file_info.clone());
                                        }
                                    }
                                    DownloadHistoryEntry::Failure => {
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
