pub mod key;

use std::path::{Path, PathBuf};

use key::*;

#[derive(Debug)]
pub struct FragmentInfo {
    pub hash: Key,
    pub path: PathBuf,
    pub size: u64,
}

impl FragmentInfo {
    pub fn new(hash: Key, path: &Path, size: u64) -> FragmentInfo {
        return FragmentInfo {
            hash,
            path: path.to_path_buf(),
            size,
        };
    }
}
