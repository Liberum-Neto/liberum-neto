use anyhow::anyhow;
use anyhow::{Error, Result};
use base64::prelude::*;
use rand::Rng;

#[derive(Debug, Clone, Copy)]
pub struct Key {
    value_bytes: [u8; 32],
}

#[allow(unused)]
impl Key {
    pub fn random() -> Key {
        let mut rng = rand::thread_rng();
        let mut u64_parts = [0u64; 4];

        for i in 0..4 {
            u64_parts[i] = rng.gen::<u64>();
        }

        u64_parts.into()
    }

    pub fn as_u64_slice_be(&self) -> [u64; 4] {
        let mut result = [0u64; 4];

        for i in 0..4 {
            let mut value_bytes_for_u64 = [0u8; 8];
            value_bytes_for_u64.copy_from_slice(&self.value_bytes[i * 8..(i + 1) * 8]);
            result[i] = u64::from_be_bytes(value_bytes_for_u64);
        }

        result
    }

    pub fn as_u8_slice_be(&self) -> [u8; 32] {
        self.value_bytes
    }

    pub fn as_base64(&self) -> String {
        BASE64_STANDARD.encode(&self.value_bytes)
    }
}

impl From<[u8; 32]> for Key {
    fn from(value_bytes: [u8; 32]) -> Self {
        Key { value_bytes }
    }
}

impl TryFrom<&[u8]> for Key {
    type Error = Error;

    fn try_from(value_bytes: &[u8]) -> Result<Self> {
        Ok(Key {
            value_bytes: value_bytes[..32].try_into()?,
        })
    }
}

impl TryFrom<Vec<u8>> for Key {
    type Error = Error;

    fn try_from(value_bytes: Vec<u8>) -> Result<Self> {
        Ok(value_bytes[..].try_into()?)
    }
}

impl From<[u64; 4]> for Key {
    fn from(value_u64s: [u64; 4]) -> Self {
        let value_bytes_vec = value_u64s
            .into_iter()
            .flat_map(|u64| u64.to_be_bytes())
            .collect::<Vec<u8>>();

        // We are sure that it will work, because there are exactly 32 bytes in vec
        value_bytes_vec.try_into().unwrap()
    }
}

impl TryFrom<&[u64]> for Key {
    type Error = Error;

    fn try_from(value_bytes: &[u64]) -> Result<Self> {
        value_bytes[..4].try_into()
    }
}

impl TryFrom<Vec<u64>> for Key {
    type Error = Error;

    fn try_from(value_bytes: Vec<u64>) -> Result<Self> {
        Ok(value_bytes[..].try_into()?)
    }
}

impl TryFrom<String> for Key {
    type Error = Error;

    fn try_from(base64_string: String) -> Result<Key> {
        let value_bytes: Vec<u8> = BASE64_STANDARD.decode(base64_string)?;

        if !value_bytes.len().cmp(&32).is_eq() {
            return Result::Err(anyhow!("number of base64 bytes is less than 32"));
        }

        Result::Ok(value_bytes.try_into()?)
    }
}

impl Into<[u64; 4]> for Key {
    fn into(self) -> [u64; 4] {
        self.value_bytes
            .iter()
            .as_slice()
            .windows(8)
            .map(|w| u64::from_be_bytes(w.try_into().unwrap()))
            .collect::<Vec<u64>>()
            .try_into()
            .unwrap()
    }
}

impl Into<Vec<u64>> for Key {
    fn into(self) -> Vec<u64> {
        <Key as Into<[u64; 4]>>::into(self).to_vec()
    }
}
