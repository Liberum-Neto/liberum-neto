use std::{fs::{self, Permissions}, os::unix::fs::PermissionsExt, path::Path, time::Duration};
use tracing::{info, error, debug};
use libp2p::swarm::Swarm;
use tokio::sync::mpsc;
pub mod configs;
use configs::Config;
use daemonize::*;
use liberum_core;
use tokio::net::UnixListener;
use liberum_core::UIMessage;

fn build_swarm(config: &Config) -> Result<libp2p::swarm::Swarm<libp2p::ping::Behaviour>, Box<dyn std::error::Error>>{
    let id = config.get_identity();

    Ok(libp2p::SwarmBuilder::with_existing_identity(id.clone())
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| libp2p::ping::Behaviour::default())?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(10)))
        .build())
}


#[tokio::main]
pub async fn run(path: &Path) -> Result<(), Box<dyn std::error::Error>>{
    let mut config: Option<Config> = None;

    let (sender, mut receiver) = mpsc::channel(16);
    let socket = path.join("liberum-core-socket");
    
    fs::remove_file(&socket).unwrap_or_else(|e|debug!("Daemon removing old socket file: {e}"));
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, Permissions::from_mode(0o666)).unwrap();
    tokio::spawn(liberum_core::listen(listener, sender.clone()));

    // Loop until a swarm can be built
    let swarm: Result<Swarm<libp2p::ping::Behaviour>, Box<dyn std::error::Error>> = loop {
        tokio::select! {
            Some(msg) = receiver.recv() => {
                debug!("CORE RECEIVED A MESSAGE {msg:?}");
                match msg {
                    UIMessage::GenerateConfig{ path } => {
                        debug!("Received GenerateConfig in core!");
                        match Config::new(path.clone()).save() {
                            Ok(_) => {},
                            Err(e) => {
                                error!("Error generating a config {e}")
                            }
                        }
                    },
                    UIMessage::LoadConfig{ path } => {
                        debug!("Received LoadConfig {path:?} in core!");
                        if let Ok(c) = Config::load(path.clone()) {
                            debug!("Successfully oaded the config at {path:?}");
                            config = Some(c);
                        }
                    }
                }

                if let Some(ref config) = config {
                    debug!("Got a config. Trying to build a swarm");
                    let s = build_swarm(&config);
                    if let Ok(s) = s {
                        break Ok(s);
                    }
                }
            }
            else => {}
        }
    };

    // Continue with the built swarm
    match swarm {
        Ok(_swarm) => {
            info!("Swarm was built from {config:?}");
            // run_core(swarm);
        },
        Err(_e) => {
            error!("This shouldn't happen. Core should run until it receives valid config.");
        }
    }

    println!("Core ends!");
    Ok(())

}


fn setup_logging() {
    tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .with_line_number(true)
    .with_target(true)
    .pretty()
    .with_file(true).init();
}


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