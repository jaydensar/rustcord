mod prisma;

use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use hmac_sha256::Hash;
use jsonwebtoken::{EncodingKey, Header};
use load_dotenv::load_dotenv;
use prisma::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;

struct State {
    prisma: PrismaClient,
}

load_dotenv!();

#[tokio::main]
async fn main() {
    let client = prisma::new_client().await;
    let prisma = client.unwrap();

    let shared_state = Arc::new(State { prisma });

    let app = Router::new()
        .route("/", get(root))
        .route("/register", post(register))
        .route("/login", post(login))
        .layer(Extension(shared_state));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root() -> &'static str {
    "rc_api v0.1.0"
}

#[derive(Deserialize)]
struct AuthPayload {
    username: String,
    password: String,
}

async fn register(
    Extension(state): Extension<Arc<State>>,
    Json(payload): Json<AuthPayload>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let mut hasher = Hash::new();

    hasher.update(payload.password);

    let created_user = prisma
        .user()
        .create(
            user::username::set(payload.username),
            user::password::set(hex::encode(hasher.finalize())),
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

#[derive(Debug, Serialize, Deserialize)]
struct TokenData {
    username: String,
    id: String,
}

async fn login(
    Extension(state): Extension<Arc<State>>,
    Json(payload): Json<AuthPayload>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let mut hasher = Hash::new();

    hasher.update(payload.password);

    let password_hash = hex::encode(hasher.finalize());

    let user_query = prisma
        .user()
        .find_unique(user::username::equals(payload.username))
        .exec()
        .await;

    println!("{:?}", user_query);

    if user_query.is_err() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found."})),
        );
    }

    let user_data = user_query.unwrap().unwrap();

    if user_data.password != password_hash {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Incorrect Password"
            })),
        );
    }

    let token_data = TokenData {
        username: user_data.username.to_owned(),
        id: user_data.id.to_owned(),
    };

    let token = jsonwebtoken::encode(
        &Header::default(),
        &token_data,
        &EncodingKey::from_secret(env!("JWT_TOKEN_SECRET").as_ref()),
    )
    .unwrap();

    (
        StatusCode::OK,
        Json(json!({
            "id": user_data.id,
            "username": user_data.username,
            "token": token
        })),
    )
}
