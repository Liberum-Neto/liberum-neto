mod node;

use anyhow::{anyhow, Result};
use daemonize::*;
use liberum_core::core_connection;
use liberum_core::messages;
use std::{
    fs::{self, Permissions},
    io,
    os::unix::fs::PermissionsExt,
    path::Path,
};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, error};

/// The main function of the core daemon
#[tokio::main]
pub async fn run(path: &Path) -> Result<()> {
    let (daemon_request_sender, mut daemon_request_receiver) = mpsc::channel(16);
    let (daemon_response_sender, daemon_response_receiver) = mpsc::channel::<String>(16);
    let config_manager = liberum_core::configs::ConfigManager::new(None).or_else(|e| {
        error!("Failed to load the config manager: {e}");
        Err(e)
    })?;
    let socket = path.join("liberum-core-socket");
    fs::remove_file(&socket).or_else(|e| {
        if e.kind() != io::ErrorKind::NotFound {
            error!("Failed to remove the old socket: {e}");
            return Err(anyhow!(e));
        }
        Ok(())
    })?;
    let listener = UnixListener::bind(&socket).or_else(|e| {
        error!("Failed to bind the socket: {e}");
        Err(anyhow!(e))
    })?;
    fs::set_permissions(&socket, Permissions::from_mode(0o666)).or_else(|e| {
        error!("Failed to set permissions on the socket: {e}");
        Err(anyhow!(e))
    })?;
    tokio::spawn(core_connection::listen(
        listener,
        daemon_request_sender.clone(),
        daemon_response_receiver,
    ));

    loop {
        tokio::select! {
            Some(msg) = daemon_request_receiver.recv() => {
                debug!("Core received a message {msg:?}");
                match msg {
                    messages::DaemonRequest::NewNode{ name } => {
                        match config_manager.add_config(&name) {
                            Ok(path) => {
                                daemon_response_sender.send(format!("Config generated config at {path:?}")).await?;
                            },
                            Err(e) => {
                                let e = format!("Failed to generate config at {path:?}: {e}");
                                daemon_response_sender.send(e.clone()).await?;
                                error!(e);
                            }
                        };
                    },
                    messages::DaemonRequest::StartNode{ name } => {
                        if let Ok(c) = config_manager.get_node_config(&name) {
                            let path = config_manager.get_node_config_path(&name);
                            debug!("Successfully loaded the config of {}", c.name);
                            daemon_response_sender.send(format!("Config loaded from {path:?}")).await?;
                            // TODO Build a swarm here and start a task that will run
                            // the swarm
                            // TODO how to communicate with the node task you want
                            // when there are multiple tasks running?
                        } else {
                            error!("Error loading the config at {path:?}");
                        }
                    },
                }
            }
            else => {}
        }
    }
}

/// Helper function to setup logging
fn setup_logging() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_line_number(true)
        .with_target(true)
        .pretty()
        .with_file(true)
        .init();
}

/// Actual main function that starts the daemon
/// Must be run without tokio runtime to start the daemon properly
/// Only after the process is daemonized, the tokio runtime is started

fn start_daemon(path: &Path) -> Result<()> {
    let uid = nix::unistd::geteuid();
    let gid = nix::unistd::getgid();

    let daemonize = Daemonize::new()
        .working_directory(path)
        .pid_file(path.join("core.pid"))
        //.chown_pid_file(true)
        .stdout(fs::File::create(path.join("stdout.out"))?)
        .stderr(fs::File::create(path.join("stderr.out"))?)
        .user(uid.as_raw())
        .group(gid.as_raw());
    debug!("Attempting to start the daemon as user {uid} group {gid}!");
    if daemonize.start().is_err() {
        return Err(anyhow!("Failed to daemonize the process"));
    }
    debug!("Daemon starts as user {uid} group {gid}!");
    Ok(())
}

fn main() -> Result<()> {
    setup_logging();
    let path = Path::new("/tmp/liberum-core/");
    match fs::remove_dir_all(path) {
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                error!("Failed to remove the directory: {e}");
                return Err(anyhow!(e));
            }
        }
        _ => {}
    }
    fs::create_dir(path)?;
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--daemon" {
        start_daemon(path)?;
    }

    match run(&path) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Error running the core daemon: {e}");
            std::process::exit(-1);
        }
    }
}
