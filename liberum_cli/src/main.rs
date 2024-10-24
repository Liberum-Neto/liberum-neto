use std::{env::temp_dir, fs, io::Write, path::{Path, PathBuf}};
use clap::{Error, Parser, Subcommand};
use tokio::net::UnixStream;
use liberum_core::{self, UIMessage};
use tracing_subscriber;
use tracing::{info, warn, error, debug};

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).init();
    //let mut socket = UnixStream::connect()).await.unwrap();
    let path = Path::new("/tmp/liberum-core/");
    let mut sender = liberum_core::connect(path.join("liberum-core-socket")).await;

    loop {
        let line = readline()?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let args = shlex::split(line).ok_or("error: Invalid quoting")?;
        let cli = Cli::try_parse_from(args).map_err(|e| e.to_string());
        let cli = match cli {
            Ok(cli) => {
                cli
            },
            Err(e) => {
                info!("Match CLI: {e}");
                continue;
            }
        };
        
        let sender = match sender.as_ref() {
            Ok(sender) => {
                sender
            }
            Err(e) => {
                error!("No connection with the core!");
                error!("{}",e.to_string());
                continue;
            }
        };

        let response: Result<bool, ()> = match cli.command {
            Commands::NewNode { path } => {
                let path = match path {
                    Some(p) => Some(PathBuf::from(p)),
                    None => None,
                };
                debug!("Creating node at {:?}", path);
                sender.send(liberum_core::UIMessage::GenerateConfig { path }).await.unwrap();
                Ok(false)
            }
            
            Commands::LoadNode { path } => {
                let path = match path {
                    Some(p) => Some(PathBuf::from(p)),
                    None => None,
                };
                debug!("Loading node config at {:?}", path);
                sender.send(liberum_core::UIMessage::LoadConfig { path }).await.unwrap();
                Ok(false)
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
                
                Ok(false)
            }
            Commands::DownloadFile { name } => {
                debug!("Download file");
                Ok(false)
            }
            Commands::Exit => {
                Ok(true)
            }
        };

        match response {
            Ok(quit) => {
                info!("quit: {quit}");
                if quit {
                    break;
                }
            }
            Err(err) => {
                write!(std::io::stdout(), "{err:?}").map_err(|e| e.to_string())?;
                std::io::stdout().flush().map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
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


fn readline() -> Result<String, String> {
    write!(std::io::stdout(), "$ ").map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .map_err(|e| e.to_string())?;
    Ok(buffer)
}