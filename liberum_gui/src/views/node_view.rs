use std::any::Any;

use crate::{
    components::status_bar::StatusBar,
    windows::{
        dialer_window::DialerWindow, download_window::DownloadWindow,
        downloader_window::DownloaderWindow, node_config_window::NodeConfigWindow,
        node_window::NodeWindow, search_result_window::SearchResultWindow,
        search_window::SearchWindow, Window,
    },
};

use super::{AppView, NodesListView, ViewAction, ViewContext};

pub struct NodeView {
    node_name: String,
    config_window: NodeConfigWindow,
    node_window: NodeWindow,
    dialer_window: DialerWindow,
    downloader_window: DownloaderWindow,
    download_window: Option<DownloadWindow>,
    search_window: SearchWindow,
    search_result_window: Option<SearchResultWindow>,
    status_line: String,
}

struct NodeViewState {
    status_line: String,
}

impl NodeView {
    pub fn new(node_name: &str) -> Self {
        Self {
            node_name: node_name.to_string(),
            config_window: NodeConfigWindow::new(node_name),
            node_window: NodeWindow::new(node_name),
            dialer_window: DialerWindow::new(node_name),
            downloader_window: DownloaderWindow::new(node_name),
            download_window: None,
            search_window: SearchWindow::new(node_name),
            search_result_window: None,
            status_line: String::new(),
        }
    }

    fn show_default_panel(&mut self, ctx: &mut ViewContext) {
        egui::CentralPanel::default().show(ctx.egui_ctx, |ui| {
            ui.heading("Node details page");
        });
    }

    fn show_node_window(&mut self, ctx: &mut ViewContext) {
        let update = self.node_window.draw(ctx);

        if update.config_button_clicked {
            self.config_window.open();
        }

        if let Some(new_status_line) = update.new_status_line {
            self.status_line = new_status_line;
        }
    }

    fn show_dialer_window(&mut self, ctx: &mut ViewContext) {
        let update = self.dialer_window.draw(ctx);

        if let Some(new_status_line) = update.new_status_line {
            self.status_line = new_status_line;
        }
    }

    fn show_downloader_window(&mut self, ctx: &mut ViewContext) {
        let update = self.downloader_window.draw(ctx);

        if let Some(new_status_line) = update.new_status_line {
            self.status_line = new_status_line;
        }

        if let Some(file_info) = update.file_downloaded {
            let mut download_window = DownloadWindow::new(file_info);
            download_window.open();
            self.download_window = Some(download_window);
        }

        if let Some(file_info) = update.display_file_info {
            let mut download_window = DownloadWindow::new(file_info);
            download_window.open();
            self.download_window = Some(download_window);
        }
    }

    fn show_config_window(&mut self, ctx: &mut ViewContext) {
        self.config_window.draw(ctx);
    }

    fn show_download_window(&mut self, ctx: &mut ViewContext) {
        if let Some(window) = &mut self.download_window {
            window.draw(ctx);
        }
    }

    fn show_status_bar(&mut self, ctx: &mut ViewContext) -> ViewAction {
        let mut action = ViewAction::Stay;

        egui::TopBottomPanel::bottom(egui::Id::new("status_bar"))
            .frame(egui::Frame::default().inner_margin(16.0))
            .show_separator_line(false)
            .show(ctx.egui_ctx, |ui| {
                ui.add(StatusBar::status(
                    &self.status_line,
                    &mut action,
                    Box::new(NodesListView::new()),
                ))
            });

        action
    }

    fn show_search_window(&mut self, ctx: &mut ViewContext) {
        let update = self.search_window.draw(ctx);

        if let Some(new_status_line) = update.new_status_line {
            self.status_line = new_status_line;
        }

        if let Some(search_result) = update.search_result {
            self.search_result_window = Some(SearchResultWindow::new(search_result));
        }
    }

    fn show_search_result_window(&mut self, ctx: &mut ViewContext) {
        if let Some(search_result_window) = &mut self.search_result_window {
            search_result_window.draw(ctx);
        }
    }
}

impl AppView for NodeView {
    fn setup(&mut self, ctx: &mut ViewContext, init_state: Option<Box<dyn Any>>) {
        ctx.system_observer
            .borrow_mut()
            .add_observed_config(&self.node_name);

        if let Some(init_state) = init_state {
            let node_view_state = init_state.downcast::<NodeViewState>().unwrap();
            self.status_line = node_view_state.status_line;
        }
    }

    fn draw(&mut self, mut ctx: &mut ViewContext) -> ViewAction {
        self.show_default_panel(&mut ctx);
        self.show_config_window(&mut ctx);
        self.show_node_window(&mut ctx);
        self.show_download_window(&mut ctx);
        self.show_dialer_window(&mut ctx);
        self.show_downloader_window(&mut ctx);
        self.show_search_window(ctx);
        self.show_search_result_window(ctx);
        self.show_status_bar(&mut ctx)
    }

    fn teardown(&mut self, ctx: &mut ViewContext) -> Option<Box<dyn Any>> {
        ctx.system_observer
            .borrow_mut()
            .remove_observed_config(&self.node_name);

        Some(Box::new(NodeViewState {
            status_line: self.status_line.clone(),
        }))
    }

    fn unique_state_id(&self) -> String {
        format!("node_view_{}", self.node_name)
    }
}
