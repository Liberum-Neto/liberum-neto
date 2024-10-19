use std::{io::Write, path::{Path, PathBuf}};
use clap::{Error, Parser, Subcommand};

const LN_CONFIG_DIRECTORY: &str = ".liberum-neto";

fn main() -> Result<(), String> {
    loop {
        let line = readline()?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match respond(line) {
            Ok(quit) => {
                if quit {
                    break;
                }
            }
            Err(err) => {
                write!(std::io::stdout(), "{err}").map_err(|e| e.to_string())?;
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

fn respond(line: &str) -> Result<bool, String> {
    let args = shlex::split(line).ok_or("error: Invalid quoting")?;
    let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
    match cli.command {
        Commands::NewNode { path } => {
            let path_str = path;

            let (path, result) = get_config_or_default(path_str, false);
            if let Err(()) = result {
                println!("{} already exits!", path.to_str().unwrap());
                return Ok(false);
            }                 
            println!("Creating node at {}", path.to_str().expect("Path should be able to be represented as string"));

            // path is valid at this point
        }
        
        Commands::LoadNode { path } => {
            let path_str = path;

            let (path, result) = get_config_or_default(path_str, true);
            if let Err(()) = result {
                println!("{} does not exit!", path.to_str().unwrap());
                return Ok(false);
            }  

            println!("Loading node config at {}", path.to_str().unwrap());
        }

        Commands::PublishFile { path, name } => {
            let path_str = path;
            let path = std::path::Path::new(&path_str);

            if !path.exists() {
                println!("File {path_str} does not exist");
            }

            let name = match name {
                Some(name) => name,
                None => {
                    String::from(path.file_name().expect("Path should contain filename").to_str().expect("Publish filename should be convertable to str"))
                }
            };

            println!("Publish file {name} at {}", path.file_name().unwrap().to_str().unwrap());
            
            
        }
        Commands::DownloadFile { name } => {
            println!("Download file");
        }
        Commands::Exit => {
            return Ok(true)
        }
    }
    Ok(false)
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