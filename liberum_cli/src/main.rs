use std::{env::temp_dir, fs, io::Write, path::{Path, PathBuf}};
use clap::{Error, Parser, Subcommand};
use tokio::net::UnixStream;
use liberum_core;

const LN_CONFIG_DIRECTORY: &str = ".liberum-neto";

#[tokio::main]
async fn main() -> Result<(), String> {

    //let mut socket = UnixStream::connect()).await.unwrap();
    let mut sender = liberum_core::connect(temp_dir().join("liberum-core-socket")).await;

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
                println!("{e}");
                continue;
            }
        };
        

        let response: Result<bool, ()> = match cli.command {
            Commands::NewNode { path } => {
                let path_str = path;
    
                let (path, result) = get_config_or_default(path_str, false);
                if let Err(()) = result {
                    println!("{} already exits!", path.to_str().unwrap());
                    fs::remove_dir(&path).unwrap();
                } //else {              
                    println!("Creating node at {}", path.to_str().expect("Path should be able to be represented as string"));
                    let sender = sender.as_ref().unwrap();
                    sender.send(liberum_core::UIMessage::GenerateConfig {  }).await.unwrap();
                    println!("Sent message!");
                    
                //}
                // path is valid at this point
                Ok(false)
            }
            
            Commands::LoadNode { path } => {
                let path_str = path;
    
                let (path, result) = get_config_or_default(path_str, true);
                if let Err(()) = result {
                    println!("{} does not exit!", path.to_str().unwrap());
                }  else {
                    println!("Loading node config at {}", path.to_str().unwrap());
                }
                Ok(false)
            }
    
            Commands::PublishFile { path, name } => {
                let path_str = path;
                let path = std::path::Path::new(&path_str);
    
                if !path.exists() {
                    println!("File {path_str} does not exist");
                } else {
                    let name = match name {
                        Some(name) => name,
                        None => {
                            String::from(path.file_name().expect("Path should contain filename").to_str().expect("Publish filename should be convertable to str"))
                        }
                    };
                println!("Publish file {name} at {}", path.file_name().unwrap().to_str().unwrap());
                }
                
                Ok(false)
            }
            Commands::DownloadFile { name } => {
                println!("Download file");
                Ok(false)
            }
            Commands::Exit => {
                Ok(true)
            }
        };

        match response {
            Ok(quit) => {
                println!("quit: {quit}");
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

/// if path_str == None => if default path exists == should_exist then the path, else None
/// if path_str == Some => if path exists == should_exist then the path, else None
fn get_config_or_default(path_str: Option<String>, should_exist: bool) -> (PathBuf, Result<(),()>) {
    let path: PathBuf;

    if let Some(p) = path_str {
        path = std::path::Path::new(&p).to_path_buf();
    } else {
        path = homedir::my_home().unwrap()
        .expect("Should be able to find the home path")
        .join(LN_CONFIG_DIRECTORY);
    }

    if path.exists() == should_exist{
        return (path, Ok(()));
    }

    (path, Err(()))
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