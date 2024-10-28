use std::{fs::{self, Permissions}, os::unix::fs::PermissionsExt, path::Path, time::Duration};
use tracing::{info, error, debug};
use libp2p::swarm::Swarm;
use tokio::sync::mpsc;
use liberum_core::configs::Config;
use daemonize::*;
use tokio::net::UnixListener;
use liberum_core::messages;
use liberum_core::core_connection;

/// The main function of the core daemon
#[tokio::main]
pub async fn run(path: &Path) -> Result<(), Box<dyn std::error::Error>>{
    let (daemon_request_sender, mut daemon_request_receiver) = mpsc::channel(16);
    let (daemon_response_sender, mut daemon_response_receiver) = mpsc::channel::<String>(16);
    let config_manager = liberum_core::configs::ConfigManager::new(None);
    let socket = path.join("liberum-core-socket");
    fs::remove_file(&socket).unwrap_or_else(|e|debug!("Daemon removing old socket file: {e}"));
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, Permissions::from_mode(0o666)).unwrap();
    tokio::spawn(core_connection::listen(listener, daemon_request_sender.clone(), daemon_response_receiver));

    
    loop {
        tokio::select! {
            Some(msg) = daemon_request_receiver.recv() => {
                debug!("Core received a message {msg:?}");
                match msg {
                    messages::DaemonRequest::NewNode{ name } => {
                        match config_manager.add_config(&name) {
                            Ok(path) => {
                                daemon_response_sender.send(format!("Config generated config at {path:?}")).await.unwrap();
                            },
                            Err(e) => {
                                let e = format!("Failed to generate config at {path:?}: {e}");
                                daemon_response_sender.send(e.clone()).await.unwrap();
                                error!(e);
                            }
                        };
                    },
                    messages::DaemonRequest::StartNode{ name } => {
                        if let Ok(c) = config_manager.get_node_config(&name) {
                            let path = config_manager.get_node_config_path(&name);
                            debug!("Successfully loaded the config of {}", c.name);
                            daemon_response_sender.send(format!("Config loaded from {path:?}")).await.unwrap();
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
    };

}

/// Helper function to setup logging
fn setup_logging() {
    tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .with_line_number(true)
    .with_target(true)
    .pretty()
    .with_file(true).init();
}


/// Actual main function that starts the daemon
/// Must be run without tokio runtime to start the daemon properly
/// Only after the process is daemonized, the tokio runtime is started
fn main() {
    setup_logging();

    let path = Path::new("/tmp/liberum-core/");
    fs::remove_dir_all(path).unwrap_or_else(|e|debug!("{e}"));
    fs::create_dir(path).unwrap_or_else(|e| debug!("{e}"));
    let args: Vec<String> = std::env::args().collect();
    let uid = nix::unistd::geteuid();
    let gid = nix::unistd::getgid();
    if args.len() > 1 && args[1] == "--daemon" {
        let daemonize = Daemonize::new()
        .working_directory(path)
        .pid_file(path.join("core.pid"))
        //.chown_pid_file(true)
        .stdout(fs::File::create(path.join("stdout.out")).unwrap())
        .stderr(fs::File::create(path.join("stderr.out")).unwrap())
        .user(uid.as_raw())
        .group(gid.as_raw());
        debug!("Attempting to start the daemon as user {uid} group {gid}!");
        daemonize.start().expect("Should daemonize");
        debug!("Daemon starts as user {uid} group {gid}!");
    }

    run(&path).unwrap();
}