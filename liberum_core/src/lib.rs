
use std::{error::Error, time::Duration};
use libp2p::futures::StreamExt;
use tracing::{debug, error, info, warn};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::net::{UnixStream, UnixListener};
use std::path::PathBuf;

use tokio_util::codec::{Decoder, Encoder};
use bytes::{Buf, BufMut, BytesMut};
use std::marker::PhantomData;
use futures::prelude::*;
use std::any::type_name;

pub struct UIActor {
    pub sender: mpsc::Sender<UIMessage>,
    pub receiver: mpsc::Receiver<UIMessage>
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UIMessage {
    GenerateConfig {
        path: Option<std::path::PathBuf>
    },
    LoadConfig {
        path: Option<std::path::PathBuf>
    }
}

impl UIActor {
    pub fn new() -> Self {
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
        dst.put(bincode::serialize::<T>(&item).unwrap().as_slice());
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
        match result {
            Ok(message) => {debug!("Deserialized"); Ok(Some(message))},
            Err(e) => {error!("Failed to deserialize {e}"); Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "fail deserializing message"))}
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

pub async fn listen(listener: UnixListener, sender: mpsc::Sender<UIMessage>, mut receiver: mpsc::Receiver<String>) {
    info!("Serwer nasluchuje na {:?}", listener);
    
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        info!("Obsługa nowego połączenia");
        let sender = sender.clone();
        let encoder: AsymmetricMessageCodec<String, UIMessage> = AsymmetricMessageCodec::new();
        let mut framed = encoder.framed(socket);
        loop {
            tokio::select! {
                Some(message) = framed.next() => {
                    info!("Received: {message:?}");
                    //framed.send(format!("Received {message:?}",)).await.unwrap();
                    match message {
                        Ok(message) => {
                            sender.send(message).await.unwrap();
                            let response = receiver.recv().await.unwrap();
                            framed.send(response).await.unwrap();
                        },
                        Err(e) => {warn!("Error receiving message: {e:?}"); break;}
                    };
                },
                else => {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

pub async fn connect(socket_path: PathBuf) -> Result<(mpsc::Sender<UIMessage>, mpsc::Receiver<String>), Box<dyn Error>> {
    let socket = UnixStream::connect(&socket_path).await?;
    let encoder: AsymmetricMessageCodec<UIMessage, String> = AsymmetricMessageCodec::new();
    let mut framed = encoder.framed(socket);
    let (sender,mut receiver) = mpsc::channel::<UIMessage>(16);
    let (resp_sender, mut resp_receiver) = mpsc::channel::<String>(16);
    tokio::spawn (async move {
        loop {
            tokio::select! {
                Some(message) = receiver.recv() => {
                    debug!("Actor received message, sending to socket");
                    framed.send(message).await.unwrap();
                    let resp = framed.next().await.unwrap().unwrap();
                    info!("Received: {}", resp);
                    resp_sender.send(resp).await.unwrap();
                }
            };
        }
    });
    Ok((sender, resp_receiver))
}