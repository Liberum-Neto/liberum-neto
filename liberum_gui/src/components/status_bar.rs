use egui::Widget;

use crate::views::{NodesListView, ViewAction};

pub struct StatusBar<'a> {
    status: Option<String>,
    view_action: &'a mut ViewAction,
}

impl<'a> StatusBar<'a> {
    pub fn empty(view_action: &'a mut ViewAction) -> StatusBar<'a> {
        return StatusBar {
            status: None,
            view_action,
        };
    }

    pub fn status(status: &str, view_action: &'a mut ViewAction) -> StatusBar<'a> {
        StatusBar {
            status: Some(status.to_string()),
            view_action,
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
                        view: Box::new(NodesListView::new()),
                    }
                }

                ui.label(&status);
            })
            .response;

        *self.view_action = action;

        resp
    }
}
