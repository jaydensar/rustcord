use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use serde_json::json;

use crate::{prisma, Claims, State};

pub async fn me(
    Extension(state): Extension<Arc<State>>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let user_query = prisma
        .user()
        .find_unique(prisma::user::username::equals(claims.username))
        .with(prisma::user::WithParam::Memberships(vec![]))
        .exec()
        .await
        .unwrap()
        .unwrap();

    (
        StatusCode::OK,
        Json(json!({
            "id": user_query.id,
            "username": user_query.username,
            "createdAt": user_query.created_at.to_string(),
            "memberships": user_query.memberships().unwrap()
        })),
    )
}

pub async fn get_user_guilds(
    Extension(state): Extension<Arc<State>>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let guilds_query = prisma
        .guild()
        .find_many(vec![prisma::guild::members::every(vec![
            prisma::guild_membership::user_id::equals(claims.id),
        ])])
        .with(prisma::guild::WithParam::Channels(vec![]))
        .exec()
        .await;

    if guilds_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        );
    }

    let guilds_data = guilds_query.unwrap();

    (StatusCode::OK, Json(json!(guilds_data)))
}
