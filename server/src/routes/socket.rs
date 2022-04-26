use std::sync::Arc;

use axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use serde::Serialize;
use serde_json::json;

use crate::{prisma, Claims, State, User};

#[derive(Debug, Clone, Serialize)]
pub struct SocketMessagePayload {
    msg_type: String,
    content: String,
    author: User,
    channel_id: String,
    created_at: String,
    id: String,
}

#[derive(Debug, Clone)]
pub enum SocketMessageType {
    NewMessage(SocketMessagePayload, String),
    GuildDataUpdate(String),
    UserGuildDataUpdate(String),
}

#[derive(Debug, Clone)]
pub struct SocketPayload {
    message: SocketMessageType,
}

pub async fn upgrade(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<State>>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state, claims))
}

async fn websocket(mut socket: WebSocket, state: Arc<State>, claims: Claims) {
    let mut rx = state.tx.subscribe();

    let user_id = &claims.id;

    let mut user_data = state
        .prisma
        .user()
        .find_unique(prisma::user::id::equals(user_id.to_owned()))
        .with(prisma::user::WithParam::Memberships(vec![]))
        .exec()
        .await
        .unwrap()
        .unwrap();

    let mut user_guild_data = state
        .prisma
        .guild()
        .find_many(vec![prisma::guild::id::in_vec(
            user_data
                .memberships()
                .unwrap()
                .iter()
                .map(|m| m.guild_id.to_owned())
                .collect(),
        )])
        .with(prisma::guild::WithParam::Channels(vec![]))
        .exec()
        .await
        .unwrap();

    loop {
        let msg = rx.recv().await;

        match msg.unwrap().message {
            SocketMessageType::NewMessage(message_payload, channel_id) => {
                if user_guild_data
                    .iter()
                    .map(|g| g.channels().unwrap())
                    .any(|channels| channels.iter().any(|channel| channel.id == channel_id))
                {
                    let socket_msg = json!(message_payload);
                    socket
                        .send(axum::extract::ws::Message::Text(socket_msg.to_string()))
                        .await
                        .ok();
                }
            }

            SocketMessageType::GuildDataUpdate(updated_guild_id) => {
                if user_guild_data.iter().any(|g| g.id == updated_guild_id) {
                    user_guild_data = state
                        .prisma
                        .guild()
                        .find_many(vec![prisma::guild::id::in_vec(
                            user_data
                                .memberships()
                                .unwrap()
                                .iter()
                                .map(|m| m.guild_id.to_owned())
                                .collect(),
                        )])
                        .with(prisma::guild::WithParam::Channels(vec![]))
                        .exec()
                        .await
                        .unwrap();

                    socket
                        .send(axum::extract::ws::Message::Text(
                            json!({
                                "msg_type": "guild_data_update",
                                "guild_id": updated_guild_id,
                            })
                            .to_string(),
                        ))
                        .await
                        .ok();
                }
            }

            SocketMessageType::UserGuildDataUpdate(updated_user_id) => {
                if *user_id == updated_user_id {
                    user_data = state
                        .prisma
                        .user()
                        .find_unique(prisma::user::id::equals(user_id.to_owned()))
                        .with(prisma::user::WithParam::Memberships(vec![]))
                        .exec()
                        .await
                        .unwrap()
                        .unwrap();

                    user_guild_data = state
                        .prisma
                        .guild()
                        .find_many(vec![prisma::guild::id::in_vec(
                            user_data
                                .memberships()
                                .unwrap()
                                .iter()
                                .map(|m| m.guild_id.to_owned())
                                .collect(),
                        )])
                        .with(prisma::guild::WithParam::Channels(vec![]))
                        .exec()
                        .await
                        .unwrap();

                    socket
                        .send(axum::extract::ws::Message::Text(
                            json!({
                                "msg_type": "user_guild_data_update",
                            })
                            .to_string(),
                        ))
                        .await
                        .ok();
                }
            }
        }
    }
}
