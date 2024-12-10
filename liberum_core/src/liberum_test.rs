use core::error;
use std::{
    collections::HashMap, fs::File, io::Write, iter::zip, panic, path::PathBuf, str::FromStr,
    sync::Arc, time::Duration, usize,
};

use connection::AppContext;
use liberum_core::{node_config::NodeConfig, DaemonError, DaemonRequest, DaemonResponse};
use libp2p::Multiaddr;
use node::store::NodeStore;
use tokio::time::sleep;
use tonic::{
    metadata::MetadataValue,
    service::Interceptor,
    transport::{Channel, Uri},
    Code,
};

use crate::test_protocol::test_scenario::node_definition::NodeDefinitionLevel;
use crate::test_protocol::test_scenario::test_part_scenario::Part::Simple;

use test_protocol::{
    action_resoult::{Details, DialNodeResult, GetObjectResult, PublishObjectResult},
    callable_nodes::CallableNode,
    identity_server_client::IdentityServerClient,
    Action, ActionResoult, DaemonQueryStats, Identity, NodeInstance, NodesCreated, TestPartResult,
    TestScenario,
};
use tracing::{error, info};
pub mod connection;
pub mod node;
pub mod swarm_runner;
pub mod test_runner;
pub mod vault;

pub mod test_protocol {
    tonic::include_proto!("test_protocol");
}

struct TestContext {
    scenario: TestScenario,
    callable_nodes: HashMap<u64, CallableNode>,
    app_context: AppContext,
}

struct HostHeaderInterceptor {
    pub host_id: String,
}

impl Interceptor for HostHeaderInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        let token = MetadataValue::from_str(&self.host_id)
            .map_err(|_| tonic::Status::new(Code::InvalidArgument, "Invalid tocken"))?;

        let mut req = request;
        req.metadata_mut().insert("host-id", token);
        return Ok(req);
    }
}

#[tokio::main]
pub(crate) async fn run_test(
    url: String,
    host_id: String,
) -> Result<(), Box<dyn core::error::Error>> {
    let app_context = connection::AppContext::new(kameo::spawn(
        NodeStore::with_custom_nodes_dir(std::env::temp_dir().as_path()).await?,
    ));
    // let mut client = IdentityServerClient::connect(url).await?;
    let channel = Channel::builder(Uri::from_str(&url)?)
        .connect()
        .await
        .unwrap();

    let interceptor = HostHeaderInterceptor {
        host_id: host_id.clone(),
    };

    let mut client = IdentityServerClient::with_interceptor(channel, interceptor);

    let test_scenario = client
        .identify(Identity { host_id: host_id })
        .await?
        .into_inner();

    let new_nodes = handle_create_nodes(&test_scenario, app_context.clone()).await;
    sleep(Duration::from_secs(1)).await;
    let diallable_nodes = client.test_ready(new_nodes).await?.into_inner();

    let mut test_context = TestContext {
        scenario: test_scenario,
        callable_nodes: HashMap::new(),
        app_context,
    };

    for node in diallable_nodes.nodes {
        test_context.callable_nodes.insert(node.node_id, node);
    }

    for file in &test_context.scenario.files {
        File::create(&PathBuf::from(file.hash.to_string()).as_path())?.write(&file.object)?;
    }

    let (result_tx, result_rx) = tokio::sync::mpsc::channel::<TestPartResult>(128);

    let mut part_stream = client
        .test_partake(tokio_stream::wrappers::ReceiverStream::new(result_rx))
        .await?
        .into_inner();

    let ctx = Arc::new(test_context);

    while let Some(descriptor) = part_stream.message().await? {
        let part = &ctx.scenario.parts[descriptor.part_id as usize];

        let mut actions_tasks = Vec::new();
        let mut action_result = Vec::new();

        if let Some(scenario) = &part.part {
            match scenario {
                Simple(test_part_simple) => {
                    for ele in &test_part_simple.actions {
                        actions_tasks
                            .push(tokio::spawn(handle_simple_action(ele.clone(), ctx.clone())));
                    }

                    for handle in actions_tasks {
                        action_result.push(handle.await?);
                    }
                }
            }
        }

        result_tx
            .send(TestPartResult {
                resoults: action_result,
            })
            .await?;
    }
    Ok(())
}

async fn handle_simple_action(action: Action, ctx: Arc<TestContext>) -> ActionResoult {
    let mut result = ActionResoult {
        action_source_id: action.action_id,
        action_start_time: chrono::Utc::now().to_rfc3339(),
        ..Default::default()
    };

    match action.details {
        Some(details) => {
            let request = match &details {
                test_protocol::action::Details::Dial(dial_node) => DaemonRequest::Dial {
                    node_name: action.node_name,
                    peer_id: ctx
                        .callable_nodes
                        .get(&dial_node.dialed_node_id)
                        .unwrap()
                        .node_hash
                        .to_string(),
                    addr: ctx
                        .callable_nodes
                        .get(&dial_node.dialed_node_id)
                        .unwrap()
                        .node_address()
                        .to_string(),
                },
                test_protocol::action::Details::PublishObject(publish_object) => {
                    DaemonRequest::PublishFile {
                        node_name: action.node_name,
                        path: PathBuf::from(publish_object.hash.to_string()),
                    }
                }
                test_protocol::action::Details::GetObject(get_object) => {
                    DaemonRequest::DownloadFile {
                        node_name: action.node_name,
                        id: get_object.object_hash.clone(),
                    }
                }
            };

            let daemon_request = daemon_request(request, ctx.app_context.clone()).await;
            result.action_stop_time = chrono::Utc::now().to_rfc3339();

            match daemon_request {
                Ok(response) => {
                    result.is_success = true;
                    result.details = Some(match response {
                        // DaemonResponse::FileProvided { id } => todo!(),
                        // DaemonResponse::Providers { ids } => todo!(),
                        DaemonResponse::FileDownloaded { data: _, stats } => {
                            if let Some(stats) = stats {
                                Details::GetObject(GetObjectResult {
                                    stats: Some(DaemonQueryStats {
                                        query_duration_in_nano: stats.query_duration.as_nanos()
                                            as u64,
                                        total_request: stats.total_requests as u64,
                                    }),
                                })
                            } else {
                                Details::GetObject(GetObjectResult { stats: None })
                            }
                        }
                        DaemonResponse::Dialed => Details::Dial(DialNodeResult {}),
                        DaemonResponse::FilePublished { id: _ } => {
                            Details::PublishObject(PublishObjectResult {})
                        }
                        _ => panic!(),
                    })
                }
                Err(error) => match error {
                    DaemonError::Other(err) => {
                        result.error = Some(err);
                        result.details = Some(match &details {
                            test_protocol::action::Details::Dial(_) => {
                                Details::Dial(DialNodeResult {})
                            }
                            test_protocol::action::Details::PublishObject(_) => {
                                Details::PublishObject(PublishObjectResult {})
                            }
                            test_protocol::action::Details::GetObject(_) => {
                                Details::GetObject(GetObjectResult { stats: None })
                            }
                        });
                    }
                    _ => panic!(),
                },
            }
        }
        None => {}
    }

    return result;
}

async fn daemon_request(
    request: DaemonRequest,
    app_context: AppContext,
) -> Result<DaemonResponse, DaemonError> {
    connection::handle_message(request, &app_context).await
}

async fn run_few_and_collect(
    requests: Vec<(u64, DaemonRequest)>,
    app_context: AppContext,
) -> Result<Vec<(u64, DaemonResponse)>, Box<dyn error::Error>> {
    let mut tasks = Vec::with_capacity(requests.len());
    sleep(Duration::from_millis(250)).await;
    for request in &requests {
        tasks.push(tokio::spawn(daemon_request(
            request.1.clone(),
            app_context.clone(),
        )));
    }

    let mut results = Vec::with_capacity(tasks.len());
    for request in zip(tasks, &requests) {
        results.push((request.1 .0, request.0.await??));
    }
    Ok(results)
}

async fn handle_create_nodes(
    test_scenario: &TestScenario,
    app_context: AppContext,
) -> NodesCreated {
    let mut create_node_requests = Vec::new();

    for node in &test_scenario.nodes {
        create_node_requests.push((
            node.node_id,
            liberum_core::DaemonRequest::NewNode {
                node_name: node.name.clone(),
                id_seed: None,
            },
        ));
    }

    run_few_and_collect(create_node_requests, app_context.clone())
        .await
        .unwrap();

    // load hashes

    let mut hash_requests = Vec::new();
    let mut dialable_nodes = HashMap::new();

    for node in &test_scenario.nodes {
        if node.visibility() == NodeDefinitionLevel::NeedHash
            || node.visibility() == NodeDefinitionLevel::NeedAddress
        {
            dialable_nodes.insert(
                node.node_id,
                NodeInstance {
                    node_id: node.node_id,
                    node_hash: "".to_owned(),
                    node_adress: None,
                    node_name: node.name.to_string(),
                },
            );
        }
    }

    let mut add_external_addr = Vec::new();
    // add external adress
    for node in &test_scenario.nodes {
        if node.visibility() == NodeDefinitionLevel::NeedAddress
        {
            add_external_addr.push((
                node.node_id,
                DaemonRequest::OverwriteNodeConfig {
                    node_name: node.name.clone(),
                    new_cfg: NodeConfig{
                        bootstrap_nodes: Vec::new(),
                        external_addresses: vec![Multiaddr::from_str("/ip4/0.0.0.0/udp/0/quic-v1").unwrap()]
                    }
                },
            ));
        }
    }
    run_few_and_collect(add_external_addr, app_context.clone())
    .await
    .unwrap();

    // start nodes

    let mut start_requests = Vec::new();

    for node in &test_scenario.nodes {
        let request = DaemonRequest::StartNode {
            node_name: node.name.clone(),
        };
        start_requests.push((node.node_id, request));
    }
    run_few_and_collect(start_requests, app_context.clone())
        .await
        .unwrap();

    //load hash after start

    for node in &test_scenario.nodes {
        if node.visibility() == NodeDefinitionLevel::NeedHash
            || node.visibility() == NodeDefinitionLevel::NeedAddress
        {
            hash_requests.push((
                node.node_id,
                DaemonRequest::GetPeerId {
                    node_name: node.name.clone(),
                },
            ));
        }
    }
    
    for response in run_few_and_collect(hash_requests, app_context.clone())
        .await
        .unwrap()
    {
        match response {
            (node_id, DaemonResponse::PeerId { id: hash }) => {
                dialable_nodes.get_mut(&node_id).unwrap().node_hash = hash
            }
            _ => panic!(),
        }
    }

    // load address

    let mut address_requests = Vec::new();

    for node in &test_scenario.nodes {
        if node.visibility() == NodeDefinitionLevel::NeedAddress {
            let request = DaemonRequest::GetNodeDetails {
                node_name: node.name.clone(),
            };

            address_requests.push((node.node_id, request));
        }
    }
    for (node_id, result) in run_few_and_collect(address_requests, app_context.clone())
        .await
        .unwrap()
    {
        if let DaemonResponse::NodeDetails(details) = result {
            dialable_nodes.get_mut(&node_id).unwrap().node_adress =
                Some(details.running_addresses.first().unwrap().clone());
        }
    }

    return NodesCreated {
        nodes: dialable_nodes.values().cloned().collect(),
    };
}

/// Helper function to setup logging
fn setup_logging() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_line_number(true)
        .with_target(true)
        .compact()
        .with_file(true)
        .with_env_filter("liberum_test=debug")
        .init();
}

fn main() -> Result<(), ()> {
    setup_logging();

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 3 && args[1] == "--test" {
        let url = args[2].clone();
        let host_id = args[3].clone();
        match run_test(url, host_id) {
            Ok(_) => {
                info!("Exited normaly")
            }
            Err(err) => error!(err, "error"),
        }
    } else {
        error!("Improper params");
    }

    Ok(())
}
