use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use liberum_core;
use std::path::Path;
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
            error!("Failed to connect to the core: ({e}). Make sure the client is running!");
            Err(anyhow!(e))?
        }
    };
    
    let cli = Cli::parse();

    match cli.command {
        Commands::NewNode { name } => {
            debug!("Creating node {name}");
            request_sender
                .send(liberum_core::messages::DaemonRequest::NewNodes { names: vec![name]  })
                .await?;
            match response_receiver.recv().await {
                Some(r) => info!("Daemon responds: {:?}", r),
                None => {
                    error!("Failed to receive response");
                }
            };
        }

        Commands::StartNode { name } => {
            debug!("Starting node {name}");
            request_sender
                .send(liberum_core::messages::DaemonRequest::StartNodes { names: vec![name] } )
                .await?;
            match response_receiver.recv().await {
                Some(r) => info!("Client responds: {:?}", r),
                None => {
                    error!("Failed to receive response");
                }
            };
        }
    };

    Ok(())
}
