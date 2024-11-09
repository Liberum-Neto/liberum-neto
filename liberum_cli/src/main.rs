use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use liberum_core::node_config::NodeConfig;
use liberum_core::{node_config::BootstrapNode, DaemonError, DaemonRequest, DaemonResponse};
use libp2p::Multiaddr;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
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
    ListNodes,
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
    #[arg()]
    name: String,
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
    AddExternalAddr(AddExternalAddr),
}

#[derive(Parser)]
struct AddBootstrapNode {
    #[arg()]
    id: String,
    addr: String,
}

#[derive(Parser)]
struct AddExternalAddr {
    #[arg()]
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
        Command::ListNodes => handle_list_nodes(req, res).await,
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
        ConfigNodeCommand::AddBootstrapNode(sub_cmd) => {
            handle_add_bootstrap_node(&cmd.name, sub_cmd, req, res).await?
        }
        ConfigNodeCommand::AddExternalAddr(sub_cmd) => {
            handle_add_external_addr(&cmd.name, sub_cmd, req, res).await?
        }
    }

    Ok(())
}

async fn handle_list_nodes(req: RequestSender, mut res: ReseponseReceiver) -> Result<()> {
    debug!("Listing nodes...");
    req.send(DaemonRequest::ListNodes)
        .await
        .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    let node_infos = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))??;

    match node_infos {
        DaemonResponse::NodeList(node_infos) => {
            println!("{:<32} {:<32}", "NAME", "IS RUNNING");
            node_infos.iter().for_each(|info| {
                println!("{:<32} {:<32}", info.name, info.is_running);
            });
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }

    Ok(())
}

async fn handle_add_bootstrap_node(
    name: &str,
    cmd: AddBootstrapNode,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    debug!(name = name, "Adding bootstrap node");
    let mut config = get_current_config(name, &req, &mut res).await?;
    let new_bootstrap_node = BootstrapNode::from_strings(&cmd.id, &cmd.addr)?;
    config.bootstrap_nodes.push(new_bootstrap_node);

    req.send(DaemonRequest::OverwriteNodeConfig {
        name: name.to_string(),
        new_cfg: config,
    })
    .await?;

    handle_response(&mut res).await
}

async fn handle_add_external_addr(
    name: &str,
    sub_cmd: AddExternalAddr,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    debug!(name = name, "Adding external address");
    let mut config = get_current_config(name, &req, &mut res).await?;
    let new_external_addr = Multiaddr::from_str(&sub_cmd.addr)?;
    config.external_addresses.push(new_external_addr);

    req.send(DaemonRequest::OverwriteNodeConfig {
        name: name.to_string(),
        new_cfg: config,
    })
    .await?;

    handle_response(&mut res).await
}

async fn get_current_config(
    node_name: &str,
    req: &RequestSender,
    res: &mut ReseponseReceiver,
) -> Result<NodeConfig> {
    req.send(DaemonRequest::GetNodeConfig {
        name: node_name.to_string(),
    })
    .await?;

    let current_config = res
        .recv()
        .await
        .ok_or(anyhow!("failed to get response"))
        .inspect_err(|e| error!(err = e.to_string(), "Failed to receive response"))?
        .inspect_err(|e| error!(err = e.to_string(), "Failed to get current config"))?;

    let config = match current_config {
        DaemonResponse::NodeConfig(cfg) => cfg,
        _ => {
            error!("Expected response to be NodeConfig, but it is not");
            bail!("Response is not NodeConfig");
        }
    };

    Ok(config)
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
