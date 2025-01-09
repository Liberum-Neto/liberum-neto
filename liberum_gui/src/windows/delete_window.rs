use super::{DeleteInfo, Window};
use egui::Color32;

pub struct DeleterWindow {
    state: DeleterWindowState,
}

impl DeleterWindow {
    pub fn new(node_name: &str) -> Self {
        Self {
            state: DeleterWindowState {
                to_delete_id: String::new(),
                node_name: node_name.to_string(),
                delete_history: Vec::new(),
                is_opened: false,
            },
        }
    }
}

#[derive(Clone)]
pub struct DeleterWindowState {
    to_delete_id: String,
    node_name: String,
    delete_history: Vec<DeleteHistoryEntry>,
    is_opened: bool,
}

#[derive(Default)]
pub struct DeleterWindowUpdate {
    pub new_status_line: Option<String>,
    pub deleted: Option<DeleteInfo>,
    pub display_delete_info: Option<DeleteInfo>,
}

#[derive(Clone)]
enum DeleteHistoryEntry {
    Success(DeleteInfo),
    Failure,
}

impl Window<DeleterWindowState, DeleterWindowUpdate> for DeleterWindow {
    fn from_state(state: DeleterWindowState) -> Self {
        Self { state }
    }

    fn get_state(&self) -> DeleterWindowState {
        self.state.clone()
    }

    fn set_state(&mut self, state: DeleterWindowState) {
        self.state = state;
    }

    fn draw(&mut self, view_ctx: &mut crate::views::ViewContext) -> DeleterWindowUpdate {
        let mut update = DeleterWindowUpdate::default();

        egui::Window::new("Deleter").show(view_ctx.egui_ctx, |ui| {
            egui::TopBottomPanel::top("deleter_controls").show_inside(ui, |ui| {
                ui.heading("Delete file");
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "File ID:");
                    ui.text_edit_singleline(&mut self.state.to_delete_id);
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        match self.state.to_delete_id.is_empty() {
                            true => {
                                update.new_status_line =
                                    Some("File ID to delete must not be empty!".to_string());
                            }
                            false => {
                                match view_ctx
                                    .daemon_com
                                    .delete_file(&self.state.node_name, &self.state.to_delete_id)
                                {
                                    Ok(delete_info) => {
                                        update.new_status_line = Some("File deleted".to_string());
                                        update.deleted = Some(delete_info.clone());
                                        self.state
                                            .delete_history
                                            .push(DeleteHistoryEntry::Success(delete_info));
                                    }
                                    Err(e) => {
                                        self.state.delete_history.push(DeleteHistoryEntry::Failure);
                                        update.new_status_line = Some(e.to_string())
                                    }
                                }
                            }
                        }
                        self.state.to_delete_id = String::new();
                    }
                });

                ui.add_space(10.0);
            });

            if !self.state.delete_history.is_empty() {
                egui::Grid::new("delete_history")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Object ID");
                        ui.label("Deleted locally");
                        ui.label("Number of successes");
                        ui.label("Number of failures");
                        ui.label("Details");
                        ui.end_row();

                        for entry in &self.state.delete_history {
                            match entry {
                                DeleteHistoryEntry::Success(delete_info) => {
                                    ui.label(delete_info.id.clone());
                                    ui.label(delete_info.deleted_locally.clone().to_string());
                                    ui.label(delete_info.number_of_successes.to_string());
                                    ui.label(delete_info.number_of_failures.to_string());

                                    if ui.button("Details").clicked() {
                                        update.display_delete_info = Some(delete_info.clone());
                                    }
                                }
                                DeleteHistoryEntry::Failure => {
                                    ui.label(self.state.to_delete_id.clone());
                                    ui.label("-");
                                    ui.label("-");
                                    ui.label("-");
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
