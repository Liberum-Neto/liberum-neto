use egui::Widget;

use crate::views::{AppView, ViewAction};

pub struct StatusBar<'a> {
    status: Option<String>,
    view_action: &'a mut ViewAction,
    prev_view: Box<dyn AppView>,
}

impl<'a> StatusBar<'a> {
    pub fn status(
        status: &str,
        view_action: &'a mut ViewAction,
        prev_view: Box<dyn AppView>,
    ) -> StatusBar<'a> {
        StatusBar {
            status: Some(status.to_string()),
            view_action,
            prev_view,
        }
    }
}

impl<'a> Widget for StatusBar<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut action = ViewAction::Stay;
        let status = self.status.unwrap_or(String::new());

        let resp = ui
            .horizontal(|ui| {
                if ui.button("Back to nodes list").clicked() {
                    action = ViewAction::SwitchView {
                        view: self.prev_view,
                    }
                }

                ui.label(&status);
            })
            .response;

        *self.view_action = action;

        resp
    }
}
