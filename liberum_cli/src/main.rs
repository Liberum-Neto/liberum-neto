use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use liberum_core::{DaemonError, DaemonRequest, DaemonResponse};
use std::path::Path;
use std::path::PathBuf;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{debug, error, info};
use tracing_subscriber;

type RequestSender = Sender<DaemonRequest>;
type ReseponseReceiver = Receiver<Result<DaemonResponse, DaemonError>>;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Subcommands for the CLI
/// They need to be matched in the main function
/// and can send messages to the daemon
#[derive(Subcommand)]
enum Command {
    /// Creates a new node
    NewNode(NewNode),
    StartNode(StartNode),
    ConfigNode(ConfigNode),
    StopNode(StopNode),
    PublishFile(PublishFile),
    GetProviders(GetProviders),
    DownloadFile(DownloadFile),
}

#[derive(Parser)]
struct NewNode {
    #[arg()]
    name: String,
}

#[derive(Parser)]
struct StartNode {
    #[arg()]
    name: String,
}

#[derive(Parser)]
struct ConfigNode {
    #[command(subcommand)]
    subcommand: ConfigNodeCommand,
}

#[derive(Parser)]
struct StopNode {
    #[arg()]
    name: String,
}

#[derive(Subcommand)]
enum ConfigNodeCommand {
    AddBootstrapNode(AddBootstrapNode),
}

#[derive(Parser)]
struct AddBootstrapNode {
    #[arg()]
    id: String,
    addr: String,
}

#[derive(Parser)]
struct PublishFile {
    #[arg()]
    node_name: String,
    #[arg()]
    path: PathBuf,
}

#[derive(Parser)]
struct GetProviders {
    #[arg()]
    node_name: String,
    #[arg()]
    id: String,
}

#[derive(Parser)]
struct DownloadFile {
    #[arg()]
    node_name: String,
    #[arg()]
    id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    let path = Path::new("/tmp/liberum-core/");
    let conn = liberum_core::connect(path.join("liberum-core-socket")).await;

    let (request_sender, response_receiver) = match conn {
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
    handle_command(cli.command, request_sender, response_receiver).await?;

    Ok(())
}

async fn handle_command(cmd: Command, req: RequestSender, res: ReseponseReceiver) -> Result<()> {
    match cmd {
        Command::NewNode(cmd) => handle_new_node(cmd, req, res).await,
        Command::StartNode(cmd) => handle_start_node(cmd, req, res).await,
        Command::ConfigNode(cmd) => handle_config_node(cmd, req, res).await,
        Command::StopNode(cmd) => handle_stop_node(cmd, req, res).await,
        Command::PublishFile(cmd) => handle_publish_file(cmd, req, res).await,
        Command::DownloadFile(cmd) => handle_download_file(cmd, req, res).await,
        Command::GetProviders(cmd) => handle_get_providers(cmd, req, res).await,
    }
}

async fn handle_new_node(
    cmd: NewNode,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    debug!(name = cmd.name, "Creating node");
    req.send(DaemonRequest::NewNode { name: cmd.name })
        .await
        .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    handle_response(&mut res).await
}

async fn handle_start_node(
    cmd: StartNode,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    debug!(name = cmd.name, "Starting node");
    req.send(DaemonRequest::StartNode { name: cmd.name })
        .await
        .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    handle_response(&mut res).await
}

async fn handle_config_node(
    cmd: ConfigNode,
    req: RequestSender,
    res: ReseponseReceiver,
) -> Result<()> {
    match cmd.subcommand {
        ConfigNodeCommand::AddBootstrapNode(cmd) => {
            handle_add_bootstrap_node(cmd, req, res).await?
        }
    }

    Ok(())
}

async fn handle_add_bootstrap_node(
    cmd: AddBootstrapNode,
    req: RequestSender,
    res: ReseponseReceiver,
) -> Result<()> {
    todo!()
}

async fn handle_publish_file(
    cmd: PublishFile,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    debug!(path = format!("{:?}", &cmd.path), "Publishing file");
    let path = std::path::absolute(&cmd.path).expect("Path to be converted into absolute path");

    req.send(DaemonRequest::PublishFile {
        node_name: cmd.node_name,
        path,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    handle_response(&mut res).await
}

async fn handle_download_file(
    cmd: DownloadFile,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::DownloadFile {
        node_name: cmd.node_name,
        id: cmd.id,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    handle_response(&mut res).await
}

async fn handle_get_providers(
    cmd: GetProviders,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::GetProviders {
        node_name: cmd.node_name,
        id: cmd.id,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    handle_response(&mut res).await
}

async fn handle_stop_node(
    cmd: StopNode,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    debug!(name = cmd.name, "Stopping node");
    req.send(DaemonRequest::StopNode { name: cmd.name })
        .await
        .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    handle_response(&mut res).await
}

async fn handle_response(
    response_receiver: &mut tokio::sync::mpsc::Receiver<Result<DaemonResponse, DaemonError>>,
) -> Result<()> {
    match response_receiver.recv().await {
        Some(Ok(DaemonResponse::FileDownloaded { data })) => {
            info!(response = String::from_utf8(data)?, "Daemon responds")
        }
        Some(r) => info!(response = format!("{r:?}"), "Daemon responds"),
        None => {
            error!("Failed to receive response");
        }
    };

    Ok(())
}
