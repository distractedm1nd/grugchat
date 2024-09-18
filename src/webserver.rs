use crate::fullnode::FullNode;
use crate::state::Message;
use crate::tx::{PublicKey, Register, SendMessage, Transaction};
use axum::{extract::State as AxumState, http::StatusCode, Json};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub(crate) struct SendMessageRequest {
    user: Vec<u8>,
    contents: String,
    channel: String,
}

#[derive(Deserialize)]
pub(crate) struct RegisterUserRequest {
    public_key: Vec<u8>,
    id: String,
}

pub(crate) async fn list_channels(AxumState(node): AxumState<Arc<FullNode>>) -> Json<Vec<String>> {
    let state = node.state.lock().await;
    Json(state.list_channels().into_iter().cloned().collect())
}

pub(crate) async fn read_channel(
    AxumState(node): AxumState<Arc<FullNode>>,
    axum::extract::Path(channel): axum::extract::Path<String>,
) -> Json<Option<Vec<Message>>> {
    let state = node.state.lock().await;
    Json(state.read_channel(channel).cloned())
}

pub(crate) async fn register_user(
    AxumState(node): AxumState<Arc<FullNode>>,
    Json(payload): Json<RegisterUserRequest>,
) -> Result<(), (StatusCode, String)> {
    let tx = Transaction::Register(Register {
        user: PublicKey {
            bytes: payload.public_key,
        },
        id: payload.id,
    });
    node.queue_transaction(tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

pub(crate) async fn send_message(
    AxumState(node): AxumState<Arc<FullNode>>,
    Json(payload): Json<SendMessageRequest>,
) -> Result<(), (StatusCode, String)> {
    let tx = Transaction::SendMessage(SendMessage {
        user: PublicKey {
            bytes: payload.user,
        },
        contents: payload.contents,
        channel: payload.channel,
    });
    node.queue_transaction(tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}
