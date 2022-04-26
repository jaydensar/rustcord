use std::sync::Arc;

use axum::{extract::Path, http::StatusCode, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use serde_json::json;

use crate::{prisma, Claims, State};

use super::socket::{SocketMessageType, SocketPayload};

#[derive(Deserialize)]
pub struct ItemCreatePayload {
    name: String,
}

pub async fn create_guild(
    Extension(state): Extension<Arc<State>>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<ItemCreatePayload>,
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

    let user_data = user_query.unwrap().unwrap();

    let guild_query = prisma
        .guild()
        .create(
            prisma::guild::name::set(payload.name),
            prisma::guild::owner::link(prisma::user::UniqueWhereParam::IdEquals(
                user_data.clone().id,
            )),
            vec![],
        )
        .exec()
        .await;

    if guild_query.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "An error occured."})),
        );
    }

    let guild_data = guild_query.unwrap();

    prisma
        .guild_membership()
        .create(
            prisma::guild_membership::user::link(prisma::user::UniqueWhereParam::IdEquals(
                user_data.clone().id,
            )),
            prisma::guild_membership::guild::link(prisma::guild::UniqueWhereParam::IdEquals(
                guild_data.clone().id,
            )),
            vec![],
        )
        .exec()
        .await
        .unwrap();

    state
        .tx
        .send(SocketPayload {
            message: SocketMessageType::UserGuildDataUpdate(user_data.clone().id),
        })
        .ok();

    (StatusCode::CREATED, Json(json!(guild_data)))
}

pub async fn delete_guild(
    Extension(state): Extension<Arc<State>>,
    Extension(claims): Extension<Claims>,
    Path(guild_id): Path<String>,
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

    let user_data = user_query.unwrap().unwrap();

    let guild_query = prisma
        .guild()
        .find_unique(prisma::guild::id::equals(guild_id.to_owned()))
        .with(prisma::guild::WithParam::Owner)
        .exec()
        .await;

    if guild_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Guild not found."})),
        );
    }

    let guild_data = guild_query.unwrap().unwrap();

    if guild_data.owner().unwrap().id != user_data.id {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You are not the owner of this guild."})),
        );
    }

    prisma
        .guild()
        .find_unique(prisma::guild::UniqueWhereParam::IdEquals(
            guild_data.clone().id,
        ))
        .delete()
        .exec()
        .await
        .unwrap();

    state
        .tx
        .send(SocketPayload {
            message: SocketMessageType::UserGuildDataUpdate(user_data.clone().id),
        })
        .ok();

    (StatusCode::OK, Json(json!(guild_data)))
}

pub async fn create_channel(
    Extension(state): Extension<Arc<State>>,
    Extension(claims): Extension<Claims>,
    Path(guild_id): Path<String>,
    Json(payload): Json<ItemCreatePayload>,
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

    let user_data = user_query.unwrap().unwrap();

    let guild_query = prisma
        .guild()
        .find_unique(prisma::guild::id::equals(guild_id))
        .with(prisma::guild::WithParam::Channels(vec![]))
        .exec()
        .await;

    if guild_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Guild not found."})),
        );
    }

    let guild_data = guild_query.unwrap().unwrap();

    if guild_data.owner_id != user_data.id {
        return (
            StatusCode::FORBIDDEN,
            Json(
                json!({"error": "You do not have management permissions in the specified guild."}),
            ),
        );
    }

    let channel_query = prisma
        .channel()
        .create(
            prisma::channel::name::set(payload.name),
            prisma::channel::guild::link(prisma::guild::UniqueWhereParam::IdEquals(
                guild_data.clone().id,
            )),
            vec![],
        )
        .exec()
        .await;

    if channel_query.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "An error occured."})),
        );
    }

    let channel_data = channel_query.unwrap();

    state
        .tx
        .send(SocketPayload {
            message: SocketMessageType::GuildDataUpdate(guild_data.id.to_owned()),
        })
        .ok();

    (StatusCode::OK, Json(json!(channel_data)))
}
