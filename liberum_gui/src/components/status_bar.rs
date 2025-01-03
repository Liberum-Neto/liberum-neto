use egui::Widget;

pub struct StatusBar {
    status: Option<String>,
}

impl StatusBar {
    pub fn empty() -> StatusBar {
        return StatusBar { status: None };
    }

    pub fn status(status: &str) -> StatusBar {
        StatusBar {
            status: Some(status.to_string()),
        }
    }
}

impl Widget for StatusBar {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        match &self.status {
            Some(status) => ui.label(status),
            None => ui.label(""),
        }
    }
}
