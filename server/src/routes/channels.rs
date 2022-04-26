use std::sync::Arc;

use axum::{extract::Path, http::StatusCode, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use serde_json::json;

use crate::{prisma, Claims, State, User};

use super::socket::{SocketMessagePayload, SocketMessageType, SocketPayload};

#[derive(Deserialize)]
pub struct MessagePayload {
    content: String,
}

pub async fn get_channel_messages(
    Path(channel_id): Path<String>,
    Extension(claims): Extension<Claims>,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let user_query = prisma
        .user()
        .find_unique(prisma::user::id::equals(claims.id))
        .with(prisma::user::WithParam::Memberships(vec![]))
        .exec()
        .await;

    if user_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found."})),
        );
    }

    let channel_query = prisma
        .channel()
        .find_unique(prisma::channel::id::equals(channel_id.to_owned()))
        .exec()
        .await;

    if channel_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Channel not found."})),
        );
    }

    let channel_data = channel_query.unwrap().unwrap();
    let user_data = user_query.unwrap().unwrap();

    let user_memberships = user_data.memberships().unwrap();

    let is_member = user_memberships
        .iter()
        .any(|membership| membership.guild_id == channel_data.guild_id);

    if !is_member {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "User is not a member of the guild."})),
        );
    }

    let messages_query = prisma
        .message()
        .find_many(vec![prisma::message::channel_id::equals(channel_id)])
        .with(prisma::message::WithParam::Author)
        .exec()
        .await;

    if messages_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Channel not found."})),
        );
    }

    let messages_data = messages_query.unwrap();

    let mut messages_user_data: Vec<serde_json::Value> = vec![];

    for message in messages_data {
        let author = message.author().unwrap();
        messages_user_data.push(json!({
            "id": message.id,
            "content": message.content,
            "created_at": message.created_at.to_rfc3339(),
            "author": {
                "id": author.id,
                "username": author.username,
            }
        }));
    }

    (StatusCode::OK, Json(json!(messages_user_data)))
}

pub async fn post_channel_messages(
    Extension(state): Extension<Arc<State>>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<MessagePayload>,
    Path(channel_id): Path<String>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let user_query = prisma
        .user()
        .find_unique(prisma::user::id::equals(claims.id))
        .with(prisma::user::WithParam::Memberships(vec![]))
        .exec()
        .await;

    if user_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found."})),
        );
    }

    let channel_query = prisma
        .channel()
        .find_unique(prisma::channel::id::equals(channel_id.to_owned()))
        .exec()
        .await;

    if channel_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Channel not found."})),
        );
    }

    let channel_data = channel_query.unwrap().unwrap();
    let user_data = user_query.unwrap().unwrap();

    let user_memberships = user_data.memberships().unwrap();

    let is_member = user_memberships
        .iter()
        .any(|membership| membership.guild_id == channel_data.guild_id);

    if !is_member {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "User is not a member of the guild."})),
        );
    }

    let message_query = prisma
        .message()
        .create(
            prisma::message::content::set(payload.content),
            prisma::message::author::link(prisma::user::UniqueWhereParam::IdEquals(user_data.id)),
            prisma::message::channel::link(prisma::channel::UniqueWhereParam::IdEquals(channel_id)),
            vec![],
        )
        .exec()
        .await;

    if message_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Message sending failed"})),
        );
    }

    let message_data = message_query.unwrap();

    state
        .tx
        .send(SocketPayload {
            message: SocketMessageType::NewMessage(
                SocketMessagePayload {
                    msg_type: "new_message".to_owned(),
                    author: User {
                        id: message_data.clone().author_id,
                        username: user_data.username,
                    },
                    content: message_data.clone().content,
                    channel_id: message_data.clone().channel_id,
                    created_at: message_data.created_at.to_rfc3339(),
                    id: message_data.clone().id,
                },
                channel_data.id,
            ),
        })
        .unwrap();

    (StatusCode::OK, Json(json!(message_data)))
}
