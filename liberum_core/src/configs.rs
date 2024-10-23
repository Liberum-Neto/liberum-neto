use libp2p::identity::Keypair;



pub struct Config {
    pub path: Option<std::path::PathBuf>,
}

impl Config {
    pub fn new() -> Self {
        Self{path: None}
    }

    pub fn get_identity(&self) -> Keypair {
        libp2p::identity::Keypair::generate_ed25519()
    }

}