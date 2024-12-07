pub mod connection;
pub mod node;
pub mod swarm_runner;
pub mod vault;

use anyhow::{anyhow, Result};
use connection::listen;
use daemonize::*;
use std::{fs::Permissions, io, os::unix::fs::PermissionsExt, path::Path};
use tokio::net::UnixListener;
use tracing::{debug, error};

/// The main function of the core daemon
#[tokio::main]
async fn run(path: &Path) -> Result<()> {
    let socket = path.join("liberum-core-socket");
    let listener = UnixListener::bind(&socket)
        .inspect_err(|e| error!(err = e.to_string(), "Failed to bind the socket"))?;
    tokio::fs::set_permissions(&socket, Permissions::from_mode(0o666))
        .await
        .inspect_err(|e| {
            error!(
                err = e.to_string(),
                "Failed to set permissions on the socket"
            )
        })?;

    listen(listener).await?;
    Ok(())
}

/// Helper function to setup logging
fn setup_logging() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_line_number(true)
        .with_target(true)
        .compact()
        .with_file(true)
        .with_env_filter("liberum_core=error")
        .init();
}

fn start_daemon(path: &Path) -> Result<()> {
    let uid = nix::unistd::geteuid();
    let gid = nix::unistd::getgid();

    let daemonize = Daemonize::new()
        .working_directory(path)
        .pid_file(path.join("core.pid"))
        .stdout(std::fs::File::create(path.join("stdout.out"))?)
        .stderr(std::fs::File::create(path.join("stderr.out"))?)
        .user(uid.as_raw())
        .group(gid.as_raw());
    debug!(
        uid = uid.as_raw(),
        gid = gid.as_raw(),
        "Attempting to start the daemon!"
    );
    if daemonize.start().is_err() {
        return Err(anyhow!("Failed to daemonize the process"));
    }
    debug!(uid = uid.as_raw(), gid = gid.as_raw(), "Daemon starts!");
    Ok(())
}

fn main() -> Result<()> {
    setup_logging();
    let path = Path::new("/tmp/liberum-core/");
    match std::fs::remove_dir_all(path) {
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                error!(err = e.to_string(), "Failed to remove the directory");
                return Err(anyhow!(e));
            }
        }
        _ => {}
    }
    std::fs::create_dir(path)?;
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--daemon" {
        start_daemon(path)?;
    }

    match run(&path) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!(err = e.to_string(), "Error running the core daemon");
            std::process::exit(-1);
        }
    }
}
