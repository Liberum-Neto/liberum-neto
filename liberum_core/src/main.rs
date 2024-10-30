mod node;

use anyhow::{anyhow, Result};
use daemonize::*;
mod connection;

use std::{
    fs::{self, Permissions},
    io,
    os::unix::fs::PermissionsExt,
    path::Path,
};
use tokio::net::UnixListener;
use tracing::{debug, error};

/// The main function of the core daemon
#[tokio::main]
pub async fn run(path: &Path) -> Result<()> {
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
    connection::listen(
        listener
    ).await?;
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
        .init();
}

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
