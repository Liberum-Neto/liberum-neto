use std::{env::Args, io::Write, path::{Path, PathBuf}};
use clap::{Parser, Subcommand};
use liberum_core;
use tokio::sync::oneshot;
use tracing_subscriber;
use tracing::{info, warn, error, debug};

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).init();
    //let mut socket = UnixStream::connect()).await.unwrap();
    let path = Path::new("/tmp/liberum-core/");
    let contact = liberum_core::connect(path.join("liberum-core-socket")).await;

    if let Err(e) = contact {
        error!("Failed to connect to the core: ({e}). Make sure the client is running!");
        return Err(e.to_string());
    }
    let (sender, mut receiver) = contact.unwrap();
    let cli = Cli::parse();

    match cli.command {
        Commands::NewNode { path } => {
            let path = match path {
                Some(p) => Some(PathBuf::from(p)),
                None => None,
            };
            debug!("Creating node at {:?}", path);
            sender.send(liberum_core::UIMessage::GenerateConfig { path }).await.unwrap();
            info!("Client responds: {}", receiver.recv().await.unwrap());
        }
        
        Commands::LoadNode { path } => {
            let path = match path {
                Some(p) => Some(PathBuf::from(p)),
                None => None,
            };
            debug!("Loading node config at {:?}", path);
            sender.send(liberum_core::UIMessage::LoadConfig { path }).await.unwrap();
            info!("Client responds: {}", receiver.recv().await.unwrap());
        }

        Commands::PublishFile { path, name } => {
            let path_str = path;
            let path = std::path::Path::new(&path_str);

            if !path.exists() {
                info!("File {path_str} does not exist");
            } else {
                let name = match name {
                    Some(name) => name,
                    None => {
                        String::from(path.file_name().expect("Path should contain filename").to_str().expect("Publish filename should be convertable to str"))
                    }
                };
            debug!("Publish file {name} at {}", path.file_name().unwrap().to_str().unwrap());
            }
            warn!("Unimplemented");
        }
        Commands::DownloadFile { name } => {
            debug!("Download file {name}");
            warn!("Unimplemented");
        }
        Commands::Exit => {
            warn!("Unimplemented");
        }
    };

    Ok(())
}

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Creates a new node
    NewNode {
        #[arg()]
        path: Option<String>,
    },
    LoadNode {
        #[arg()]
        path: Option<String>,
    },
    PublishFile {
        #[arg()]
        path: String,
        #[arg()]
        name: Option<String>,
    },
    DownloadFile {
        #[arg()]
        name: String,
    },
    Exit,
}