use std::{collections::HashMap, usize};

use connection::AppContext;
use liberum_core::{DaemonError, DaemonRequest, DaemonResponse};
use node::store::NodeStore;

use test_protocol::{callable_nodes::CallableNode, identity_server_client::IdentityServerClient, Action, ActionResoult, Identity, NodesCreated, TestPartResult, TestScenario};
use tracing::error;
pub mod connection;
pub mod node;
pub mod swarm_runner;
pub mod test_runner;

pub mod test_protocol {
    tonic::include_proto!("test_protocol");
}


struct TestContext{
    scenario: TestScenario,
    callable_nodes: HashMap<u64,CallableNode>
}


#[tokio::main]
pub(crate) async fn run_test(url: String, host_id: String) -> Result<(),Box<dyn core::error::Error>>{

    let app_context = connection::AppContext::new(kameo::spawn(NodeStore::with_custom_nodes_dir(std::env::temp_dir().as_path()).await?));

    let mut client = IdentityServerClient::connect(url).await?;
    
    let test_scenario = client.identify(Identity{host_id:host_id}).await?.into_inner();
    
    let new_nodes = handle_create_nodes(&test_scenario,app_context.clone()).await;

    let diallable_nodes = client.test_ready(new_nodes).await?.into_inner();

    let mut test_context = TestContext{ scenario: test_scenario, callable_nodes: HashMap::new() };

    for node in diallable_nodes.nodes {
        test_context.callable_nodes.insert(node.node_id, node);
    }

    let (result_tx,result_rx)  = tokio::sync::mpsc::channel::<TestPartResult>(128);

    let mut part_stream = client.test_partake(tokio_stream::wrappers::ReceiverStream::new(result_rx)).await?.into_inner();
    
    while let Some(descriptor) = part_stream.message().await? {
        
        let part  = &test_context.scenario.parts[descriptor.part_id as usize];

        let mut actions_tasks = Vec::new();
        let mut action_result = Vec::new();


        if let Some(scenario) = &part.part{
            match scenario {
                test_protocol::test_part_scenario::Part::Simple(test_part_simple) => {
                    for ele in &test_part_simple.actions {
                        actions_tasks.push(tokio::spawn(handle_simple_action(ele.clone())));
                    }

                    for handle in actions_tasks {
                        action_result.push(handle.await?);
                    }
                },
            }


        }



        result_tx.send(TestPartResult{
            resoults: action_result
        }).await?;

       
    }
    Ok(())
}

async fn handle_simple_action(action: Action) -> ActionResoult{

    let _ = action;
    return ActionResoult{..Default::default()};
}

async fn daemon_request(request: DaemonRequest, app_context: AppContext) -> Result<DaemonResponse, DaemonError>{
    connection::handle_message(request, &app_context).await
}

async fn handle_create_nodes<'a>(test_scenario: &TestScenario, app_context: AppContext) -> NodesCreated {
    
    let mut create_node_tasks = Vec::new();

    for node in &test_scenario.nodes {
        create_node_tasks.push(
            tokio::spawn( daemon_request(liberum_core::DaemonRequest::NewNode { node_name: node.name.clone(), id_seed: None }, app_context.clone()) ));
    }

    for task in  create_node_tasks {
        let _ = task.await.unwrap();
    }

    return NodesCreated{
     ..Default::default()
    }
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


fn main() -> Result<(),()> {
    setup_logging();

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 3 && args[1] == "--test" {
        let url = args[2].clone();
        let host_id = args[3].clone();
        match run_test(url,host_id)  {
            Ok(_) => {},
            Err(err) =>         error!(
                err,
                "error"
            )
        }

    }else{
        error!(
            "Improper params"
        );
    }

    Ok(())
}
