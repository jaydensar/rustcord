use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use hmac_sha256::Hash;
use jsonwebtoken::{EncodingKey, Header};
use serde::Deserialize;
use serde_json::json;

use crate::{prisma, Claims, State};

#[derive(Deserialize)]
pub struct AuthPayload {
    username: String,
    password: String,
}

pub async fn register(
    Extension(state): Extension<Arc<State>>,
    Json(payload): Json<AuthPayload>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let mut hasher = Hash::new();

    hasher.update(payload.password);

    let created_user = prisma
        .user()
        .create(
            prisma::user::username::set(payload.username),
            prisma::user::password::set(hex::encode(hasher.finalize())),
            vec![],
        )
        .exec()
        .await;

    if created_user.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "An error occured."})),
        );
    }

    let created_user_data = created_user.unwrap();

    (
        StatusCode::CREATED,
        Json(json!({
            "id": created_user_data.id,
            "username": created_user_data.username,
            "createdAt": created_user_data.created_at.to_string(),
        })),
    )
}

pub async fn login(
    Extension(state): Extension<Arc<State>>,
    Json(payload): Json<AuthPayload>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let mut hasher = Hash::new();

    hasher.update(payload.password);

    let password_hash = hex::encode(hasher.finalize());

    let user_query = prisma
        .user()
        .find_unique(prisma::user::username::equals(payload.username))
        .exec()
        .await;

    if user_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        );
    }

    let user_data = user_query.unwrap().unwrap();

    if user_data.password != password_hash {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Incorrect password"
            })),
        );
    }

    let access_token = jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            username: user_data.username.to_owned(),
            id: user_data.id.to_owned(),
            exp: (chrono::offset::Utc::now().timestamp() + (7 * 86400)) as usize,
        },
        &EncodingKey::from_secret(env!("JWT_SECRET").as_ref()),
    )
    .unwrap();

    (
        StatusCode::OK,
        Json(json!({
            "id": user_data.id,
            "username": user_data.username,
            "token": access_token
        })),
    )
}
