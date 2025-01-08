use egui::{Align2, Color32};

use super::{PlainFileInfo, Window};

pub struct SearchWindow {
    state: SearchWindowState,
}

#[derive(Clone)]
pub struct SearchWindowState {
    is_opened: bool,
    id_to_search: String,
    node_name: String,
}

#[derive(Default)]
pub struct SearchWindowUpdate {
    pub new_status_line: Option<String>,
    pub search_result: Option<Vec<PlainFileInfo>>,
}

impl SearchWindow {
    pub fn new(node_name: &str) -> Self {
        Self {
            state: SearchWindowState {
                is_opened: true,
                id_to_search: String::new(),
                node_name: node_name.to_string(),
            },
        }
    }
}

impl Window<SearchWindowState, SearchWindowUpdate> for SearchWindow {
    fn from_state(state: SearchWindowState) -> Self {
        SearchWindow { state }
    }

    fn get_state(&self) -> SearchWindowState {
        return self.state.clone();
    }

    fn draw(&mut self, view_ctx: &mut crate::views::ViewContext) -> SearchWindowUpdate {
        let mut update = SearchWindowUpdate::default();

        egui::Window::new("Search")
            .anchor(Align2::RIGHT_CENTER, [-16.0, 0.0])
            .show(view_ctx.egui_ctx, |ui| {
                ui.label("Search objects pinned to a given object ID (hash)");
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0, 100, 200), "Object ID:");
                    ui.text_edit_singleline(&mut self.state.id_to_search);
                });
                ui.add_space(10.0);

                if ui.button("Search").clicked() {
                    let pinned_objs = view_ctx
                        .daemon_com
                        .get_pinned(&self.state.node_name, &self.state.id_to_search);
                    match pinned_objs {
                        Ok(objs) => {
                            update.search_result = Some(objs);
                        }
                        Err(e) => {
                            update.new_status_line =
                                Some(format!("Error while searching: {}", e.to_string()));
                        }
                    }
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
