use egui::{Color32, RichText};

use super::{FileInfo, Window};

pub struct DownloadWindow {
    state: DownloadWindowState,
}

#[derive(Clone)]
pub struct DownloadWindowState {
    file_info: FileInfo,
    is_opened: bool,
}

impl DownloadWindow {
    pub fn new(file_info: FileInfo) -> Self {
        Self {
            state: DownloadWindowState {
                file_info,
                is_opened: false,
            },
        }
    }
}

impl Window<DownloadWindowState, ()> for DownloadWindow {
    fn from_state(state: DownloadWindowState) -> Self {
        Self { state }
    }

    fn get_state(&self) -> DownloadWindowState {
        self.state.clone()
    }

    fn draw(&mut self, view_ctx: &mut crate::views::ViewContext) -> () {
        egui::Window::new("Download info")
            .open(&mut self.state.is_opened)
            .show(view_ctx.egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Object ID:");
                    ui.label(self.state.file_info.id.as_str());
                });
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Save path:");
                    ui.label(&self.state.file_info.path.display().to_string());
                });
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Object size:");
                    ui.label(&self.state.file_info.size.to_string());
                });
                ui.add_space(10.0);
                ui.colored_label(
                    Color32::from_rgb(0, 200, 100),
                    RichText::heading("Pinned objects:".into()),
                );

                if !self.state.file_info.pins.is_empty() {
                    egui::Grid::new("downloaded_object_pins")
                        .num_columns(1)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Pinned object ID");
                            ui.end_row();

                            for obj_id in &self.state.file_info.pins {
                                ui.label(obj_id.to_string());
                                ui.end_row();
                            }
                        });
                } else {
                    ui.label("No pinned objects found");
                }

                ui.add_space(20.0);
            });
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
