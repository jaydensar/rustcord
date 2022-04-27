use std::sync::Arc;

use axum::{extract::Path, http::StatusCode, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use serde_json::json;

use crate::{prisma, State};

use super::socket::{SocketMessageType, SocketPayload};

#[derive(Deserialize)]
pub struct ItemCreatePayload {
    name: String,
}

#[derive(Deserialize)]
pub struct InvitePayload {
    code: String,
}

pub async fn create_guild(
    Extension(state): Extension<Arc<State>>,
    Extension(user_data): Extension<prisma::user::Data>,
    Json(payload): Json<ItemCreatePayload>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

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
    Extension(user_data): Extension<prisma::user::Data>,
    Path(guild_id): Path<String>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

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
    Extension(user_data): Extension<prisma::user::Data>,
    Path(guild_id): Path<String>,
    Json(payload): Json<ItemCreatePayload>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

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

pub async fn create_invite(
    Extension(state): Extension<Arc<State>>,
    Extension(user_data): Extension<prisma::user::Data>,
    Path(guild_id): Path<String>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

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

    let invite_query = prisma
        .invite()
        .create(
            prisma::invite::creator::link(prisma::user::UniqueWhereParam::IdEquals(user_data.id)),
            prisma::invite::guild::link(prisma::guild::UniqueWhereParam::IdEquals(guild_data.id)),
            vec![],
        )
        .exec()
        .await;

    let invite_data = invite_query.unwrap();

    (StatusCode::OK, Json(json!(invite_data)))
}

pub async fn join_guild(
    Extension(state): Extension<Arc<State>>,
    Extension(user_data): Extension<prisma::user::Data>,
    Json(payload): Json<InvitePayload>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let invite_data = prisma
        .invite()
        .find_unique(prisma::invite::code::equals(payload.code))
        .with(prisma::invite::WithParam::Guild)
        .exec()
        .await
        .unwrap()
        .unwrap();

    let guild_data = invite_data.guild().unwrap();

    let membership_query = prisma
        .guild_membership()
        .create(
            prisma::guild_membership::user::link(prisma::user::UniqueWhereParam::IdEquals(
                user_data.id,
            )),
            prisma::guild_membership::guild::link(prisma::guild::UniqueWhereParam::IdEquals(
                guild_data.clone().id,
            )),
            vec![],
        )
        .exec()
        .await;

    if membership_query.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "An error occured."})),
        );
    }

    let membership_data = membership_query.unwrap();

    state
        .tx
        .send(SocketPayload {
            message: SocketMessageType::GuildDataUpdate(guild_data.clone().id),
        })
        .ok();

    (StatusCode::OK, Json(json!(membership_data)))
}
