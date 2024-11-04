use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use liberum_core::messages::DaemonRequest;
use liberum_core::{self, messages::DaemonResponse};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info};
use tracing_subscriber;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Subcommands for the CLI
/// They need to be matched in the main function
/// and can send messages to the daemon
#[derive(Debug, Subcommand)]
enum Commands {
    /// Creates a new node
    NewNode {
        #[arg()]
        name: String,
    },
    StartNode {
        #[arg()]
        name: String,
    },
    StopNode {
        #[arg()]
        name: String,
    },
    PublishFile {
        #[arg()]
        node_name: String,
        #[arg()]
        path: PathBuf,
    },
    GetProviders {
        #[arg()]
        node_name: String,
        #[arg()]
        id: String,
    },
    DownloadFile {
        #[arg()]
        node_name: String,
        #[arg()]
        id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    let path = Path::new("/tmp/liberum-core/");
    let contact = liberum_core::connect(path.join("liberum-core-socket")).await;

    let (request_sender, mut response_receiver) = match contact {
        Ok(c) => c,
        Err(e) => {
            error!(
                err = e.to_string(),
                "Failed to connect to the core. Make sure the client is running!"
            );
            Err(anyhow!(e))?
        }
    };

    let cli = Cli::parse();

    match cli.command {
        Commands::NewNode { name } => {
            debug!("Creating node {name}");
            request_sender
                .send(DaemonRequest::NewNode { name })
                .await
                .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;
        }

        Commands::StartNode { name } => {
            debug!("Starting node {name}");
            request_sender
                .send(DaemonRequest::StartNode { name })
                .await
                .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;
        }

        Commands::StopNode { name } => {
            debug!(name = name, "Stopping node");
            request_sender
                .send(DaemonRequest::StopNode { name })
                .await
                .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;
        }
        Commands::PublishFile { node_name, path } => {
            debug!(path = format!("{path:?}"), "Publishing file");
            let path = std::path::absolute(path).expect("Path to be converted into absolute path");

            request_sender
                .send(DaemonRequest::PublishFile { node_name, path })
                .await
                .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

            if let Some(Ok(resp)) = response_receiver.recv().await {
                match resp {
                    DaemonResponse::FilePublished { id } => debug!(id = id, "File published"),
                    _ => error!("{resp:?}"),
                }
            } else {
                error!("No response to publish file!");
            }
        }
        Commands::DownloadFile { node_name, id } => {
            debug!(id = format!("{id}"), "Downloading file unimplemented!");
        }
        Commands::GetProviders { node_name, id } => {
            request_sender
                .send(DaemonRequest::GetProviders { node_name, id })
                .await
                .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

            if let Some(Ok(resp)) = response_receiver.recv().await {
                match resp {
                    DaemonResponse::Providers { ids } => {
                        debug!(ids = format!("{ids:?}"), "File published")
                    }
                    _ => error!("{resp:?}"),
                }
            } else {
                error!("No response to get providers!");
            }
        }
    };

    match response_receiver.recv().await {
        Some(r) => info!(response = format!("{r:?}"), "Daemon responds: {:?}", r),
        None => {
            error!("Failed to receive response");
        }
    };

    Ok(())
}
