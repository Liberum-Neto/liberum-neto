use std::fmt::Display;

use anyhow::anyhow;
use anyhow::{Error, Result};
use liberum_core::proto::Hash;
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

    pub fn as_base58(&self) -> String {
        bs58::encode(&self.value_bytes).into_string()
    }

    pub fn from_base58(input: String) -> Result<Key> {
        let key: Key = Key::try_from(input)?;
        Ok(key)
    }

    pub fn to_hash(&self) -> Result<Hash> {
        Hash::try_from(&self.value_bytes)
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key_b58 = bs58::encode(self.value_bytes).into_string();
        write!(f, "{key_b58}")
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.value_bytes == other.value_bytes
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

impl From<[i64; 4]> for Key {
    fn from(value_i64s: [i64; 4]) -> Self {
        let value_bytes_vec = value_i64s
            .into_iter()
            .flat_map(|i| i.to_be_bytes())
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

    fn try_from(base58_string: String) -> Result<Key> {
        let vaule_bytes = bs58::decode(base58_string).into_vec()?;

        if !vaule_bytes.len().cmp(&32).is_eq() {
            return Result::Err(anyhow!("number of base58 bytes is less than 32"));
        }

        Result::Ok(vaule_bytes.try_into()?)
    }
}

impl Into<[u64; 4]> for Key {
    fn into(self) -> [u64; 4] {
        self.value_bytes
            .iter()
            .as_slice()
            .chunks(8)
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

impl Into<[i64; 4]> for Key {
    fn into(self) -> [i64; 4] {
        self.value_bytes
            .iter()
            .as_slice()
            .chunks(8)
            .map(|w| i64::from_be_bytes(w.try_into().unwrap()))
            .collect::<Vec<i64>>()
            .try_into()
            .unwrap()
    }
}
