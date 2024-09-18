use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum Transaction {
    SendMessage(SendMessage),
    Register(Register),
}

#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub bytes: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SendMessage {
    pub user: PublicKey,
    pub contents: String,
    pub channel: String,
    // signature: Signature
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Register {
    pub user: PublicKey,
    pub id: String,
    // signature: Signature
}
