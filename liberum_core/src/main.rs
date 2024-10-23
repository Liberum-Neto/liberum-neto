use std::{env::temp_dir, error::Error, time::Duration};
use tracing_subscriber::EnvFilter;
use libp2p::{futures::StreamExt, identity::Keypair, swarm::{SwarmEvent, Swarm}, Multiaddr, ping::{Behaviour}};
use std::{io, net::TcpListener};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::{mpsc, oneshot}};
use tokio::io::Interest;
use tokio::net::UnixStream;
pub mod configs;
use configs::Config;
use tokio_util::codec::{self, LinesCodec, Decoder, Encoder};
use bytes::{BytesMut, Buf, Bytes};
use std::mem;
use std::marker::PhantomData;
use futures::prelude::*;
use daemonize::*;
use liberum_core;


fn build_swarm(config: &Config) -> libp2p::swarm::Swarm<libp2p::ping::Behaviour>{
    let id = config.get_identity();

    libp2p::SwarmBuilder::with_existing_identity(id.clone())
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| libp2p::ping::Behaviour::default()).unwrap()
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(10)))
        .build()
}

#[tokio::main]
pub async fn run() {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let id: Option<Keypair> = None;
    let mut config: Option<Config> = None;
    let mut swarm: Option<Swarm<Behaviour>> = None;

    let (sender, mut receiver) = mpsc::channel(16);
    let socket = temp_dir().as_path().join("liberum-core-socket");
    tokio::spawn(liberum_core::listen(socket, sender.clone()));

    loop {
        tokio::select! {
            Some(msg) = receiver.recv() => {
                //println!("CORE RECEIVED A MESSAGE {msg:?}");
                match msg {
                    GenerateConfig => {
                        //println!("Received GenerateConfig in core!");
                        //config = Some(Config::new());
                        //swarm = Some(build_swarm(&config.unwrap()));
                    }
                }

                if swarm.is_some() {
                    break;
                }
            }
        }
    }
    println!("Core ends!")

}



fn main() {
    let daemonize = Daemonize::new()
    .pid_file("/tmp/test.pid")
    .chown_pid_file(true)
    .working_directory("/tmp")
    .user("nobody")
    .group("daemon")
    .group(2)
    .umask(0o777);

    //daemonize.start().expect("Should daemonize");

    run();
}