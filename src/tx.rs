use ed25519_dalek::{Signature as Ed25519Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Transaction {
    SendMessage(SendMessage),
    Register(Register),
}

impl Transaction {
    pub fn signature(&self) -> Signature {
        match self {
            Transaction::SendMessage(SendMessage { signature, .. }) => signature.clone(),
            Transaction::Register(Register { signature, .. }) => signature.clone(),
        }
    }

    pub fn pubkey(&self) -> PublicKey {
        match self {
            Transaction::SendMessage(SendMessage { user, .. }) => user.clone(),
            Transaction::Register(Register { user, .. }) => user.clone(),
        }
    }

    pub fn without_signature(&self) -> Transaction {
        match self {
            Transaction::SendMessage(SendMessage {
                user,
                contents,
                channel,
                ..
            }) => Transaction::SendMessage(SendMessage {
                user: user.clone(),
                contents: contents.clone(),
                channel: channel.clone(),
                signature: Signature(Vec::new()),
            }),
            Transaction::Register(Register { user, id, .. }) => Transaction::Register(Register {
                user: user.clone(),
                id: id.clone(),
                signature: Signature(Vec::new()),
            }),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
pub struct PublicKey(Vec<u8>);

impl PublicKey {
    pub fn new(bytes: Vec<u8>) -> Self {
        PublicKey(bytes)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.clone()
    }
}

impl From<VerifyingKey> for PublicKey {
    fn from(vk: VerifyingKey) -> Self {
        PublicKey(vk.to_bytes().to_vec())
    }
}

impl From<&PublicKey> for VerifyingKey {
    fn from(pk: &PublicKey) -> Self {
        VerifyingKey::from_bytes(&pk.to_bytes().try_into().unwrap()).unwrap()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Signature(Vec<u8>);

impl From<Ed25519Signature> for Signature {
    fn from(sig: Ed25519Signature) -> Self {
        Signature(sig.to_bytes().to_vec())
    }
}

impl From<&Signature> for Ed25519Signature {
    fn from(sig: &Signature) -> Self {
        Ed25519Signature::from_bytes(&sig.to_bytes().try_into().unwrap())
    }
}

impl Signature {
    pub fn new(bytes: Vec<u8>) -> Self {
        Signature(bytes)
    }

    pub fn verify(&self, pk: &PublicKey, msg: &[u8]) -> bool {
        let vk: VerifyingKey = pk.into();
        vk.verify(msg, &self.into()).is_ok()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.clone()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SendMessage {
    pub user: PublicKey,
    pub contents: String,
    pub channel: String,
    pub signature: Signature,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Register {
    pub user: PublicKey,
    pub id: String,
    pub signature: Signature,
}
