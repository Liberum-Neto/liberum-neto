
use std::{env::temp_dir, error::Error, time::Duration};
use libp2p::{futures::StreamExt, identity::Keypair, swarm::{SwarmEvent, Swarm}, Multiaddr, ping::{Behaviour}};
use tracing::{debug, info, warn, error};
use std::{io, net::TcpListener};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{fs, io::AsyncWriteExt, sync::{mpsc, oneshot}};
use tokio::io::Interest;
use tokio::net::{UnixStream, UnixListener};
use std::path::PathBuf;

use tokio_util::codec::{self, LinesCodec, Decoder, Encoder};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::mem;
use std::marker::PhantomData;
use futures::{channel::mpsc::Receiver, prelude::*};
use std::any::type_name;

pub struct UIActor {
    pub sender: mpsc::Sender<UIMessage>,
    pub receiver: mpsc::Receiver<UIMessage>
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UIMessage {
    GenerateConfig {
        path: Option<std::path::PathBuf>,
    },
    LoadConfig {
        path: Option<std::path::PathBuf>,
    }
}

impl UIActor {
    pub fn new(path: &str) -> Self {
        let (sender, receiver) = mpsc::channel(16);
        UIActor {
            sender,
            receiver
        }
    }
}

pub struct AsymmetricMessageCodec<T, U> {
    encoded_type: PhantomData<T>,
    decoded_type: PhantomData<U>
}

impl<T, U> Encoder<T> for AsymmetricMessageCodec<T, U>
where
    T: Serialize + DeserializeOwned,
    U: Serialize + DeserializeOwned,
{
    type Error = std::io::Error;

    fn encode(
        &mut self,
        item: T,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        //println!("Encoding {} bytes of {}", std::mem::size_of::<T>(), type_name::<T>());
        let len= dst.len();
        dst.put(bincode::serialize::<T>(&item).unwrap().as_slice());
        //println!("Encoded {} bytes",dst.len() - len);
        Ok(())
    }
}

impl<T, U> Decoder for AsymmetricMessageCodec<T, U>
where
    T: Serialize + DeserializeOwned,
    U: Serialize + DeserializeOwned,
{
    type Item = U;
    type Error = std::io::Error;

    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() == 0 {
            return Ok(None);
        }
        debug!("Decoding {} bytes of {}", src.len(), type_name::<U>());
        let result = bincode::deserialize::<U>(&src);
        src.advance(src.len());
        //src.clear();
        match result {
            Ok(message) => {/*println!("Deserialized");*/ Ok(Some(message))},
            Err(e) => {/*println!("Failed to deserialize {e}");*/ Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "fail deserializing message"))}
        }
    }
}

impl<T, U> AsymmetricMessageCodec<T, U> {
    pub fn new() -> Self {
        AsymmetricMessageCodec {
            encoded_type: PhantomData,
            decoded_type: PhantomData,
        }
    }
}

pub async fn listen(listener: UnixListener, sender: mpsc::Sender<UIMessage>) {
    info!("Serwer nasluchuje na {:?}", listener);
    
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        info!("Obsługa nowego połączenia");
        let sender = sender.clone();
        tokio::spawn(async move {
            let encoder: AsymmetricMessageCodec<String, UIMessage> = AsymmetricMessageCodec::new();
            let mut framed = encoder.framed(socket);
            loop {
                tokio::select! {
                    Some(message) = framed.next() => {
                        info!("Received: {message:?}");
                        framed.send(format!("Received {message:?}",)).await.unwrap();
                        match message {
                            Ok(message) => {sender.send(message).await.unwrap()},
                            Err(e) => {}
                        };
                    },
                    else => {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        
    }
}

pub async fn connect(socket_path: PathBuf) -> Result<mpsc::Sender<UIMessage>, Box<dyn Error>> {
    let socket = UnixStream::connect(&socket_path).await?;
    let encoder: AsymmetricMessageCodec<UIMessage, String> = AsymmetricMessageCodec::new();
    let mut framed = encoder.framed(socket);
    let (sender,mut receiver) = mpsc::channel::<UIMessage>(16);
    tokio::spawn (async move {
        loop {
            tokio::select! {
                Some(message) = receiver.recv() => {
                    debug!("Actor received message, sending to socket");
                    framed.send(message).await.unwrap();
                    let resp = framed.next().await.unwrap().unwrap();
                    info!("Received: {}", resp);
                }
            };
        }
    });
    Ok(sender)
}