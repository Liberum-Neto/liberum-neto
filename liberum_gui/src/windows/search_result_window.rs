use super::{PlainFileInfo, Window};

pub struct SearchResultWindow {
    state: SearchResultWindowState,
}

#[derive(Clone)]
pub struct SearchResultWindowState {
    is_opened: bool,
    search_result: Vec<PlainFileInfo>,
}

pub struct SearchResultWindowUpdate;

impl SearchResultWindow {
    pub fn new(objects: Vec<PlainFileInfo>) -> Self {
        Self {
            state: SearchResultWindowState {
                is_opened: true,
                search_result: objects,
            },
        }
    }
}

impl Window<SearchResultWindowState, SearchResultWindowUpdate> for SearchResultWindow {
    fn from_state(state: SearchResultWindowState) -> Self {
        Self { state }
    }

    fn get_state(&self) -> SearchResultWindowState {
        self.state.clone()
    }

    fn set_state(&mut self, state: SearchResultWindowState) {
        self.state = state;
    }

    fn draw(&mut self, view_ctx: &mut crate::views::ViewContext) -> SearchResultWindowUpdate {
        egui::Window::new("Search Result")
            .open(&mut self.state.is_opened)
            .show(view_ctx.egui_ctx, |ui| {
                for obj in &self.state.search_result {
                    ui.vertical(|ui| {
                        ui.label("Object");
                        ui.label(format!("ID: {}", &obj.obj_id));
                        for pin in &obj.pins {
                            ui.label(format!("Pin: {}", &pin));
                        }
                        ui.label(format!("Name: {}", &obj.name));
                        ui.add_space(10.0);
                    });
                }

                if self.state.search_result.is_empty() {
                    ui.label("No pinned objects found!");
                }
            });

        SearchResultWindowUpdate {}
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
