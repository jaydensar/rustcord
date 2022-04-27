use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use serde_json::json;

use crate::{prisma, State};

pub async fn me(Extension(user_data): Extension<prisma::user::Data>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "id": user_data.id,
            "username": user_data.username,
            "createdAt": user_data.created_at.to_string(),
            "memberships": user_data.memberships().unwrap()
        })),
    )
}

pub async fn get_user_guilds(
    Extension(state): Extension<Arc<State>>,
    Extension(user_data): Extension<prisma::user::Data>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let guilds_query = prisma
        .guild()
        .find_many(vec![prisma::guild::members::some(vec![
            prisma::guild_membership::user_id::equals(user_data.id),
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
