use super::{Signature, TypedObject, UUIDTyped};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedObject {
    pub object: TypedObject,
    pub signature: Signature,
}
impl SignedObject {
    pub const UUID: Uuid = uuid!("0193a7bf-fb8f-7fdc-8be6-02d3a3cc7eb1");
}
impl UUIDTyped for SignedObject {
    fn get_type_uuid(&self) -> Uuid {
        SignedObject::UUID
    }
}
impl SignedObject {
    pub fn sign_ed25519(object: TypedObject, keypair: libp2p::identity::Keypair) -> Result<Self> {
        let v: Vec<u8> = object.clone().try_into()?;
        let signature = Signature {
            bytes: keypair.sign(v.as_slice()).map_err(|e| anyhow!(e))?,
        };
        Ok(Self { object, signature })
    }
    pub fn verify_ed25519(&self, public: libp2p::identity::PublicKey) -> Result<bool> {
        let msg: Vec<u8> = self.object.clone().try_into()?;
        Ok(public.verify(msg.as_slice(), &self.signature.bytes.as_slice()))
    }
}

impl TypedObject {}
