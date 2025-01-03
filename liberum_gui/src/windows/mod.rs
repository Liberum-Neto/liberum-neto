use crate::views::ViewContext;

pub mod node_config_window;

pub trait Window<State, Update> {
    fn from_state(state: State) -> Self;
    // Update may be a type, which contains updated State and/or events
    fn draw(&mut self, view_ctx: &mut ViewContext) -> Update;
    fn is_opened(&self) -> bool;
    fn open(&mut self);
    fn close(&mut self);
}
