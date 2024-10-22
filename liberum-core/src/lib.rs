use std::{error::Error, time::Duration};
use tracing_subscriber::EnvFilter;
use libp2p::{futures::StreamExt, identity::Keypair, swarm::{SwarmEvent, Swarm}, Multiaddr, ping::{Behaviour}};
use std::{io, net::TcpListener};
use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, mpsc};
use tokio::io::Interest;
use tokio::net::UnixStream;
pub mod configs;
use configs::Config;


pub struct UIActor {
    pub sender: mpsc::Sender<UIMessage>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UIMessage {
    GenerateConfig {
    },
    LoadConfig {
        path: std::path::PathBuf,
    }
}

impl UIActor {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(16);
        tokio::spawn(run(receiver));
        UIActor {
            sender,
        }
        
    }
}

async fn listen(stdin: &str, stdout: &str, sender: mpsc::Sender<UIMessage>) {
    let stdin = UnixStream::connect(stdin).await.expect("Should open stdin stream");
    let stdout = UnixStream::connect(stdout).await.expect("Should open stdout");
    
    loop {
        tokio::select! {
            Ok(()) = stdin.readable() => {
                let mut data = vec![0];
                match stdin.try_read(&mut data) {
                    Ok(n) => {
                        let message: UIMessage = bincode::deserialize(&data).expect("Should deserialize message");
                        println!("{:?}", message);
                        sender.send(message);
                        
                    },
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        continue;
                    },
                    Err(e) => {
                        println!("{:?}", e);
                    }
                }
            }
        }
    }
}




fn build_swarm(config: &Config) -> libp2p::swarm::Swarm<libp2p::ping::Behaviour>{
    let id = config.get_identity();

    libp2p::SwarmBuilder::with_existing_identity(id.clone())
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| libp2p::ping::Behaviour::default()).unwrap()
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(10)))
        .build()
}

pub async fn run(mut ui: mpsc::Receiver<UIMessage>) {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let id: Option<Keypair> = None;
    let mut config: Option<Config> = None;
    let mut swarm: Option<Swarm<Behaviour>> = None;

    loop {
        tokio::select! {
            Some(msg) = ui.recv() => {
                match msg {
                    GenerateConfig => {
                        config = Some(Config::new());
                        swarm = Some(build_swarm(&config.unwrap()));
                    }
                }

                if swarm.is_some() {
                    break;
                }
            }
        }
    }

    let mut id = id.unwrap();
    let mut swarm: Swarm<Behaviour> = swarm.unwrap();

    loop {
        tokio::select! {
            Some(msg) = ui.recv() => {
                match msg {
                    GenerateConfig => {
                        
                    }
                }
            },

            p2p_msg = swarm.select_next_some() => {
                match p2p_msg {
                    SwarmEvent::NewListenAddr { listener_id, address } => println!("{listener_id} Listening on {address}"),
                    SwarmEvent::Behaviour(event) => println!("Event: {event:?}"),
                    e => {
                        //println!("{e:?}")
                    }
                }
            }
        }
    }
}
