use std::sync::Arc;

use axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use serde::Serialize;
use serde_json::json;

use crate::{prisma, State, User};

#[derive(Debug, Clone, Serialize)]
pub struct SocketMessagePayload {
    pub(crate) msg_type: String,
    pub(crate) content: String,
    pub(crate) author: User,
    pub(crate) channel_id: String,
    pub(crate) created_at: String,
    pub(crate) id: String,
}

#[derive(Debug, Clone)]
pub enum SocketMessageType {
    NewMessage(SocketMessagePayload, String),
    GuildDataUpdate(String),
    UserGuildDataUpdate(String),
}

#[derive(Debug, Clone)]
pub struct SocketPayload {
    pub(crate) message: SocketMessageType,
}

pub async fn upgrade(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<State>>,
    Extension(user_data): Extension<prisma::user::Data>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state, user_data))
}

async fn websocket(mut socket: WebSocket, state: Arc<State>, mut user_data: prisma::user::Data) {
    let mut rx = state.tx.subscribe();

    let user_id = user_data.clone().id;

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
