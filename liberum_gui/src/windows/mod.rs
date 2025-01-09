use std::path::PathBuf;

use anyhow::{bail, Result};
use liberum_core::{
    parser::{parse_typed, ObjectEnum},
    proto::{Hash, TypedObject},
};
use tracing::debug;

use crate::views::ViewContext;

pub mod delete_window;
pub mod dialer_window;
pub mod download_window;
pub mod downloader_window;
pub mod node_config_window;
pub mod node_list_window;
pub mod node_window;
pub mod search_result_window;
pub mod search_window;

pub trait Window<State, Update> {
    fn from_state(state: State) -> Self;
    fn get_state(&self) -> State;
    fn set_state(&mut self, state: State);
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
#[derive(Clone)]
pub struct DeleteInfo {
    pub id: String,
    pub deleted_locally: bool,
    pub number_of_successes: u32,
    pub number_of_failures: u32,
}

#[derive(Clone)]
pub struct PlainFileInfo {
    pub obj_id: String,
    pub name: String,
    pub data: Vec<u8>,
    pub pins: Vec<String>,
}

impl TryFrom<TypedObject> for PlainFileInfo {
    type Error = anyhow::Error;

    fn try_from(typed: TypedObject) -> Result<Self> {
        let mut typed = Some(typed);
        let mut pins = vec![];

        while let Some(obj) = typed.clone() {
            let obj_id = Hash::try_from(&obj)?.to_string();

            typed = match parse_typed(obj) {
                Err(e) => {
                    debug!("{e}");
                    continue;
                }
                Ok(obj_enum) => match obj_enum {
                    ObjectEnum::Signed(signed) => Some(signed.object),
                    ObjectEnum::PlainFile(file) => {
                        return Ok(PlainFileInfo {
                            obj_id,
                            name: file.name.clone(),
                            data: file.content,
                            pins,
                        });
                    }
                    ObjectEnum::Pin(pin) => {
                        pins.push(pin.pinned_id.to_string());

                        Some(pin.object)
                    }
                    _ => {
                        debug!("Received object was not a file!");
                        bail!("Received unsupported object type");
                    }
                },
            }
        }
        bail!("Didn't receive a supported object type in the object cascade");
    }
}

// pub enum WindowAction<State, Update, W: Window<State, Update>> {
//     OpenOtherWindow {
//         window: W,
//         state: PhantomData<State>,
//         update: PhantomData<Update>,
//     },
// }
