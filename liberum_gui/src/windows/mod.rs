use std::path::PathBuf;

use crate::views::ViewContext;

pub mod dialer_window;
pub mod download_window;
pub mod downloader_window;
pub mod node_config_window;
pub mod node_list_window;
pub mod node_window;

pub trait Window<State, Update> {
    fn from_state(state: State) -> Self;
    fn get_state(&self) -> State;
    // Update may be a type, which contains updated State and/or events
    fn draw(&mut self, view_ctx: &mut ViewContext) -> Update;
    fn is_opened(&self) -> bool;
    fn open(&mut self);
    fn close(&mut self);
}

#[derive(Clone)]
pub struct FileInfo {
    id: String,
    path: PathBuf,
    size: usize,
    pins: Vec<String>,
}

// pub enum WindowAction<State, Update, W: Window<State, Update>> {
//     OpenOtherWindow {
//         window: W,
//         state: PhantomData<State>,
//         update: PhantomData<Update>,
//     },
// }
