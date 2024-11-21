use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use liberum_core::node_config::NodeConfig;
use liberum_core::types::NodeInfo;
use liberum_core::{node_config::BootstrapNode, DaemonError, DaemonRequest, DaemonResponse};
use libp2p::Multiaddr;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{debug, error, info};
use tracing_subscriber;

type RequestSender = Sender<DaemonRequest>;
type ReseponseReceiver = Receiver<Result<DaemonResponse, DaemonError>>;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(long, short)]
    debug_log: bool,
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
    ProvideFile(ProvideFile),
    GetProviders(GetProviders),
    DownloadFile(DownloadFile),
    DownloadFileRR(DownloadFileRequestResponse),
    GetPeerID(GetPeerID),
    Dial(Dial),
    PublishFile(PublishFile),
}

#[derive(Parser)]
struct NewNode {
    #[arg()]
    name: String,
    /// WARNING - the seed is as dangerous as the private key
    /// Not recommended to use outside of testing
    #[arg(long)]
    id_seed: Option<String>,
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
struct ProvideFile {
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

#[derive(Parser)]
struct DownloadFileRequestResponse {
    #[arg()]
    node_name: String,
    #[arg()]
    id: String,
}

#[derive(Parser)]
struct GetPeerID {
    #[arg()]
    node_name: String,
}

#[derive(Parser)]
struct Dial {
    #[arg()]
    node_name: String,
    #[arg()]
    peer_id: String,
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

#[derive(Tabled)]
struct NodeInfoRow {
    pub name: String,
    pub is_running: bool,
    pub first_address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
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

    if cli.debug_log {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }

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
        Command::ProvideFile(cmd) => handle_provide_file(cmd, req, res).await,
        Command::DownloadFile(cmd) => handle_download_file(cmd, req, res).await,
        Command::DownloadFileRR(cmd) => handle_download_file_request_response(cmd, req, res).await,
        Command::GetProviders(cmd) => handle_get_providers(cmd, req, res).await,
        Command::GetPeerID(cmd) => handle_get_peer_id(cmd, req, res).await,
        Command::Dial(cmd) => handle_dial(cmd, req, res).await,
        Command::PublishFile(cmd) => handle_publish_file(cmd, req, res).await,
    }
}

async fn handle_new_node(
    cmd: NewNode,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    debug!(name = cmd.name, "Creating node");
    req.send(DaemonRequest::NewNode {
        node_name: cmd.name,
        id_seed: cmd.id_seed,
    })
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
    req.send(DaemonRequest::StartNode {
        node_name: cmd.name,
    })
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
            let node_info_rows = node_infos
                .iter()
                .map(|info| info.into())
                .collect::<Vec<NodeInfoRow>>();
            let mut table = Table::new(node_info_rows);
            table.with(Style::modern());
            let table = table.to_string();
            println!("{table}");
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
        node_name: name.to_string(),
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
        node_name: name.to_string(),
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
        node_name: node_name.to_string(),
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
    req.send(DaemonRequest::StopNode {
        node_name: cmd.name,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    handle_response(&mut res).await
}

async fn handle_provide_file(
    cmd: ProvideFile,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    debug!(path = format!("{:?}", &cmd.path), "Providing file");
    let path = std::path::absolute(&cmd.path).expect("Path to be converted into absolute path");

    req.send(DaemonRequest::ProvideFile {
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
    req.send(DaemonRequest::DownloadFileDHT {
        node_name: cmd.node_name,
        id: cmd.id,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    let response = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))??;

    match response {
        DaemonResponse::FileDownloaded { data } => {
            println!("{}", String::from_utf8(data)?);
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }

    Ok(())
}

async fn handle_download_file_request_response(
    cmd: DownloadFileRequestResponse,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::DownloadFileRequestResponse {
        node_name: cmd.node_name,
        id: cmd.id,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    let response = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))??;

    match response {
        DaemonResponse::FileDownloaded { data } => {
            println!("{}", String::from_utf8(data)?);
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }

    Ok(())
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

    let response = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))?;
    match response {
        Ok(DaemonResponse::Providers { ids }) => {
            for provider in ids {
                println!("{provider}");
            }
        }
        Err(e) => {
            error!(err = e.to_string(), "Error publishing file");
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }

    Ok(())
}

async fn handle_get_peer_id(
    cmd: GetPeerID,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::GetPeerId {
        node_name: cmd.node_name,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    let response = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))??;

    match response {
        DaemonResponse::PeerId { id } => {
            println!("{id}");
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }

    Ok(())
}

async fn handle_dial(cmd: Dial, req: RequestSender, mut res: ReseponseReceiver) -> Result<()> {
    req.send(DaemonRequest::Dial {
        node_name: cmd.node_name,
        peer_id: cmd.peer_id,
        addr: cmd.addr,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    handle_response(&mut res).await
}

async fn handle_publish_file(
    cmd: PublishFile,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::PublishFile {
        node_name: cmd.node_name,
        path: cmd.path,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    let resp = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))?;
    match resp {
        Ok(DaemonResponse::FilePublished { id }) => {
            info!(id = id, "File published");
        }
        Err(e) => {
            println!("Error publishing file: {e}");
            bail!("Error publishing file");
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }
    Ok(())
}

async fn handle_response(
    response_receiver: &mut tokio::sync::mpsc::Receiver<Result<DaemonResponse, DaemonError>>,
) -> Result<()> {
    match response_receiver.recv().await {
        Some(r) => info!(response = format!("{r:?}"), "Daemon responds"),
        None => {
            error!("Failed to receive response");
        }
    };

    Ok(())
}

impl From<&NodeInfo> for NodeInfoRow {
    fn from(value: &NodeInfo) -> Self {
        Self {
            name: value.name.to_string(),
            is_running: value.is_running,
            first_address: value
                .addresses
                .first()
                .unwrap_or(&"N/A".to_string())
                .to_string(),
        }
    }
}
