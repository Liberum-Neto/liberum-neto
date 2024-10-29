use bytes::{Buf, BufMut, BytesMut};
use serde::{de::DeserializeOwned, Serialize};
use std::any::type_name;
use std::marker::PhantomData;
use tokio_util::codec::{Decoder, Encoder};
use tracing::{debug, error};

/// A codec to use a byte stream to encode and decode messages of different types
/// - create a stream of structs from a stream of bytes
pub struct AsymmetricMessageCodec<T, U> {
    encoded_type: PhantomData<T>,
    decoded_type: PhantomData<U>,
}

impl<T, U> Encoder<T> for AsymmetricMessageCodec<T, U>
where
    T: Serialize + DeserializeOwned,
    U: Serialize + DeserializeOwned,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let serialized = bincode::serialize::<T>(&item).or_else(|e| {
            error!("Failed to serialize {e}");
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "fail serializing message",
            ))
        })?;
        dst.put(serialized.as_slice());
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

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() == 0 {
            return Ok(None);
        }
        debug!("Decoding {} bytes of {}", src.len(), type_name::<U>());
        let result = bincode::deserialize::<U>(&src);
        src.advance(src.len());
        match result {
            Ok(message) => Ok(Some(message)),
            Err(e) => {
                error!("Failed to deserialize {e}");
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "fail deserializing message",
                ))
            }
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
