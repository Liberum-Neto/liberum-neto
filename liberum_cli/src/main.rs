use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use liberum_core::node_config::NodeConfig;
use liberum_core::parser::{parse_typed, ObjectEnum};
use liberum_core::proto;
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
    #[arg(long, short)]
    machine_readable: bool,
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
    GetNodeDetails(GetNodeDetails),
    GetNodeAddresses(GetNodeAddresses),
    StopNode(StopNode),
    ProvideFile(ProvideFile),
    GetProviders(GetProviders),
    DownloadFile(DownloadFile),
    GetPeerID(GetPeerID),
    Dial(Dial),
    PublishFile(PublishFile),
    GetPublishedObjects(GetPublishedObjects),
    DeleteObject(DeleteObject),
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
struct GetNodeDetails {
    #[arg()]
    name: String,
}

#[derive(Parser)]
struct GetNodeAddresses {
    #[arg()]
    name: String,
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

#[derive(Parser)]
struct GetPublishedObjects {
    #[arg()]
    node_name: String,
}

#[derive(Parser)]
struct DeleteObject {
    #[arg()]
    node_name: String,
    #[arg()]
    object_id: String,
}

#[derive(Tabled)]
struct NodeInfoRow {
    pub name: String,
    pub peer_id: String,
    pub is_running: bool,
    pub first_cfg_address: String,
    pub first_run_address: String,
}

#[derive(Tabled)]
struct TypedObjectInfoRow {
    pub id: String,
}

struct HandlerContext {
    machine_readable: bool,
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

    let ctx = HandlerContext {
        machine_readable: cli.machine_readable,
    };
    handle_command(ctx, cli.command, request_sender, response_receiver).await?;

    Ok(())
}

async fn handle_command(
    ctx: HandlerContext,
    cmd: Command,
    req: RequestSender,
    res: ReseponseReceiver,
) -> Result<()> {
    match cmd {
        Command::NewNode(cmd) => handle_new_node(cmd, req, res).await,
        Command::StartNode(cmd) => handle_start_node(cmd, req, res).await,
        Command::ConfigNode(cmd) => handle_config_node(cmd, req, res).await,
        Command::ListNodes => handle_list_nodes(ctx, req, res).await,
        Command::GetNodeDetails(cmd) => handle_get_node_details(ctx, cmd, req, res).await,
        Command::GetNodeAddresses(cmd) => handle_get_node_addresses(ctx, cmd, req, res).await,
        Command::StopNode(cmd) => handle_stop_node(cmd, req, res).await,
        Command::ProvideFile(cmd) => handle_provide_file(cmd, req, res).await,
        Command::DownloadFile(cmd) => handle_download_file(cmd, req, res).await,
        Command::GetProviders(cmd) => handle_get_providers(cmd, req, res).await,
        Command::GetPeerID(cmd) => handle_get_peer_id(cmd, req, res).await,
        Command::Dial(cmd) => handle_dial(cmd, req, res).await,
        Command::PublishFile(cmd) => handle_publish_file(cmd, req, res).await,
        Command::GetPublishedObjects(cmd) => handle_get_published_objects(ctx, cmd, req, res).await,
        Command::DeleteObject(cmd) => handle_delete_object(cmd, req, res).await,
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

async fn handle_list_nodes(
    ctx: HandlerContext,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
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

            if ctx.machine_readable {
                table.with(Style::blank());
            } else {
                table.with(Style::modern());
            }

            let table = table.to_string();
            println!("{table}");
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }

    Ok(())
}

async fn handle_get_node_details(
    ctx: HandlerContext,
    cmd: GetNodeDetails,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::GetNodeDetails {
        node_name: cmd.name,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    let node_infos = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))??;

    match node_infos {
        DaemonResponse::NodeDetails(details) => {
            let details_row = vec![NodeInfoRow::from(&details)];
            let mut table = Table::new(details_row);

            if ctx.machine_readable {
                table.with(Style::blank());
            } else {
                table.with(Style::modern());
            }

            let table = table.to_string();
            println!("{table}");
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }

    Ok(())
}

async fn handle_get_node_addresses(
    _: HandlerContext,
    cmd: GetNodeAddresses,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::GetNodeDetails {
        node_name: cmd.name,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    let node_infos = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))??;

    match node_infos {
        DaemonResponse::NodeDetails(details) => {
            for addr in details.running_addresses {
                println!("{}", addr);
            }
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

    let response = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))?;

    match response {
        Ok(DaemonResponse::FileProvided { id }) => {
            info!(id = id, "File provided");
            println!("{id}");
            return Ok(());
        }
        Err(e) => {
            println!("Error providing file: {e}");
            bail!("Error providing file");
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }
}

async fn handle_download_file(
    cmd: DownloadFile,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::GetObject {
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
        Ok(DaemonResponse::ObjectDownloaded { data, stats: _ }) => {
            let mut typed = Some(data);
            while let Some(obj) = typed.clone() {
                typed = match parse_typed(obj).await {
                    Err(e) => {
                        debug!("{e}");
                        continue;
                    }
                    Ok(obj_enum) => match obj_enum {
                        ObjectEnum::Signed(signed) => Some(signed.object),
                        ObjectEnum::PlainFile(file) => {
                            println!("{}", String::from_utf8(file.content)?);
                            return Ok(());
                        }
                        _ => {
                            debug!("Received object was not a file!");
                            bail!("Received unsupported object type");
                        }
                    },
                }
            }
            bail!("Didn't receive a supported object type in the object cascade");
        }
        Err(DaemonError::Other(_)) => {
            println!("Failed to download file");
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
        Ok(DaemonResponse::Providers { ids, .. }) => {
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
        peer_id: cmd.peer_id.clone(),
        addr: cmd.addr.clone(),
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    match res.recv().await {
        Some(Ok(r)) => {
            info!(response = format!("{r:?}"), "Daemon responds");
            println!("Dialing successful");
        }
        Some(Err(e)) => {
            error!(err = e.to_string(), "Error dialing peer");
            println!("Error dialing peer");
        }
        None => {
            error!("Failed to receive response");
        }
    };
    Ok(())
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
            println!("{id}");
            return Ok(());
        }
        Err(e) => {
            println!("Error publishing file: {e}");
            bail!("Error publishing file");
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }
}

async fn handle_get_published_objects(
    ctx: HandlerContext,
    cmd: GetPublishedObjects,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::GetPublishedObjects {
        node_name: cmd.node_name,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    let resp = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))?;
    match resp {
        Ok(DaemonResponse::PublishedObjectsList {
            object_infos: files,
        }) => {
            let file_info_rows = files
                .iter()
                .map(|info| info.into())
                .collect::<Vec<TypedObjectInfoRow>>();
            let mut table = Table::new(file_info_rows);

            if ctx.machine_readable {
                table.with(Style::blank());
            } else {
                table.with(Style::modern());
            }

            let table = table.to_string();
            println!("{table}");
        }
        Err(e) => {
            println!("Error getting published files list: {e}");
            bail!("Error getting published files list");
        }
        _ => {
            bail!("Daemon returned wrong response");
        }
    }
    Ok(())
}

async fn handle_delete_object(
    cmd: DeleteObject,
    req: RequestSender,
    mut res: ReseponseReceiver,
) -> Result<()> {
    req.send(DaemonRequest::DeleteObject {
        node_name: cmd.node_name,
        object_id: cmd.object_id,
    })
    .await
    .inspect_err(|e| error!(err = e.to_string(), "Failed to send message"))?;

    let resp = res
        .recv()
        .await
        .ok_or(anyhow!("Daemon returned no response"))?;
    match resp {
        Ok(DaemonResponse::ObjectDeleted {
            deleted_myself,
            deleted_count,
            failed_count,
        }) => {
            println!("Deleted myself: {deleted_myself}");
            println!("Successful deletes: {deleted_count}");
            println!("Failed deletes: {failed_count}");
        }
        Err(e) => {
            println!("Error deleting object: {e}");
            bail!("Error deleting object");
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
        Some(r) => {
            info!(response = format!("{r:?}"), "Daemon responds");

            if let Err(e) = r {
                return Err(e.into());
            }
        }
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
            peer_id: value.peer_id.clone(),
            is_running: value.is_running,
            first_cfg_address: value
                .config_addresses
                .first()
                .unwrap_or(&"N/A".to_string())
                .to_string(),
            first_run_address: value
                .running_addresses
                .first()
                .unwrap_or(&"N/A".to_string())
                .to_string(),
        }
    }
}

impl From<&proto::Hash> for TypedObjectInfoRow {
    fn from(value: &proto::Hash) -> Self {
        Self {
            id: value.to_string(),
        }
    }
}
