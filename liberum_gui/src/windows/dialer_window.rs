use egui::Align2;

use super::Window;

pub struct DialerWindow {
    state: DialerWindowState,
}

impl DialerWindow {
    pub fn new(node_name: &str) -> Self {
        Self {
            state: DialerWindowState {
                dial_peer: PeerInfo::default(),
                node_name: node_name.to_string(),
                dial_history: Vec::new(),
                is_opened: false,
            },
        }
    }
}

#[derive(Clone)]
pub struct DialerWindowState {
    dial_peer: PeerInfo,
    node_name: String,
    dial_history: Vec<DialHistoryEntry>,
    is_opened: bool,
}

pub struct DialerWindowUpdate {
    pub new_status_line: Option<String>,
}

impl Default for DialerWindowUpdate {
    fn default() -> Self {
        Self {
            new_status_line: None,
        }
    }
}

#[derive(Clone, Default)]
struct PeerInfo {
    id: String,
    addr: String,
}

#[derive(Clone)]
struct DialHistoryEntry {
    peer: PeerInfo,
    successful: bool,
}

impl Window<DialerWindowState, DialerWindowUpdate> for DialerWindow {
    fn from_state(state: DialerWindowState) -> Self {
        DialerWindow { state }
    }

    fn get_state(&self) -> DialerWindowState {
        self.state.clone()
    }

    fn draw(&mut self, view_ctx: &mut crate::views::ViewContext) -> DialerWindowUpdate {
        let mut update = DialerWindowUpdate::default();

        egui::Window::new("Dialer")
            .anchor(Align2::RIGHT_TOP, [-16.0, 16.0])
            .show(view_ctx.egui_ctx, |ui| {
                egui::TopBottomPanel::top("dial_controls").show_inside(ui, |ui| {
                    ui.label("PeerID:");
                    ui.text_edit_singleline(&mut self.state.dial_peer.id);
                    ui.label("Peer address:");
                    ui.text_edit_singleline(&mut self.state.dial_peer.addr);

                    ui.add_space(10.0);

                    if ui.button("Dial").clicked() {
                        match view_ctx.daemon_com.dial(
                            &self.state.node_name,
                            &self.state.dial_peer.id,
                            &self.state.dial_peer.addr,
                        ) {
                            Ok(_) => {
                                update.new_status_line = Some(format!(
                                    "Dial {} @ {} successful!",
                                    self.state.dial_peer.id, self.state.dial_peer.addr
                                ));

                                self.state.dial_history.push(DialHistoryEntry {
                                    peer: self.state.dial_peer.clone(),
                                    successful: true,
                                });

                                self.state.dial_peer = PeerInfo::default();
                            }
                            Err(e) => {
                                update.new_status_line = Some(e.to_string());
                                self.state.dial_history.push(DialHistoryEntry {
                                    peer: self.state.dial_peer.clone(),
                                    successful: false,
                                });
                            }
                        }
                    }

                    ui.add_space(10.0);
                });

                if !self.state.dial_history.is_empty() {
                    egui::Grid::new("dial_history")
                        .num_columns(3)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Peer ID");
                            ui.label("Peer address");
                            ui.label("Successful?");
                            ui.end_row();

                            for entry in &self.state.dial_history {
                                ui.label(&entry.peer.id);
                                ui.label(&entry.peer.addr);
                                ui.label(entry.successful.to_string());
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
