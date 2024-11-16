pub mod key;

use key::*;

#[derive(Debug)]
pub struct Fragment {
    pub hash: Key,
    pub path: String,
    pub size: u64,
}

impl Fragment {
    pub fn new(hash: Key, path: String, size: u64) -> Fragment {
        return Fragment { hash, path, size };
    }

    pub fn random() -> Fragment {
        Fragment::new(Key::random(), "".to_string(), 1)
    }
}
