use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::Result;
use egui::Color32;
use egui_file::FileDialog;
use liberum_core::{
    proto::{file::PlainFileObject, pins::PinObject, Hash, TypedObject},
    types::NodeInfo,
};

use super::Window;

pub struct NodeWindow {
    state: NodeWindowState,
    file_to_send_dialog: Option<FileDialog>,
}

#[derive(Clone)]
pub struct NodeWindowState {
    node_name: String,
    file_to_send_path: Option<PathBuf>,
    input_pin_hash: String,
    pins_to_send: HashSet<String>,
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
                input_pin_hash: String::new(),
                pins_to_send: HashSet::new(),
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

    fn set_state(&mut self, state: NodeWindowState) {
        self.state = state;
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
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "PeerId:");
                    ui.label(&node_info.peer_id);
                });

                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Is running:");
                    ui.label(&node_info.is_running.to_string());
                });

                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Config addresses:");

                    ui.vertical(|ui| {
                        for addr in &node_info.config_addresses {
                            ui.label(addr);
                        }
                    });

                    if node_info.config_addresses.is_empty() {
                        ui.label("No addresses");
                    }
                });

                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Running addresses:");

                    ui.vertical(|ui| {
                        for addr in &node_info.running_addresses {
                            ui.label(addr);
                        }
                    });

                    if node_info.running_addresses.is_empty() {
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

                ui.colored_label(Color32::from_rgb(0, 200, 100), "Pins to include:");

                if !self.state.pins_to_send.is_empty() {
                    egui::Grid::new("pins_to_send")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            self.state.pins_to_send.clone().iter().for_each(|pin| {
                                ui.label(pin);

                                if ui.button("Remove").clicked() {
                                    self.state.pins_to_send.remove(pin);
                                }

                                ui.end_row();
                            });
                        });
                }

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.state.input_pin_hash);

                    if ui.button("Add pin").clicked() && !self.state.input_pin_hash.is_empty() {
                        self.state
                            .pins_to_send
                            .insert(self.state.input_pin_hash.clone());
                        self.state.input_pin_hash.clear();
                    }
                });

                ui.add_space(10.0);

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
                                let object: TypedObject =
                                    PlainFileObject::try_from_path_sync(&path).unwrap().into();

                                let obj_res =
                                    prepare_pin_object(object, self.state.pins_to_send.clone());

                                match obj_res {
                                    Ok(object) => {
                                        let result = view_ctx
                                            .daemon_com
                                            .publish_object(&self.state.node_name, object);

                                        match result {
                                            Ok(id) => {
                                                update.new_status_line =
                                                    Some(format!("File published; id={id}"));
                                                self.state.file_to_send_path = None;
                                                self.state.pins_to_send.clear();
                                            }
                                            Err(e) => update.new_status_line = Some(e.to_string()),
                                        };
                                    }
                                    Err(e) => {
                                        update.new_status_line =
                                            Some(format!("Failed to preapre pin object: {}", e));

                                        return;
                                    }
                                }
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

fn prepare_pin_object(typed_object: TypedObject, pins: HashSet<String>) -> Result<TypedObject> {
    let mut pin_hashes = vec![];

    for pin in pins.iter() {
        pin_hashes.push(Hash::try_from(pin)?);
    }

    Ok(PinObject::add_pins(
        typed_object,
        pin_hashes
            .into_iter()
            .map(|pin_hash| (pin_hash, None))
            .collect(),
    ))
}
