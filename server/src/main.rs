mod prisma;

use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use prisma::*;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;

struct State {
    prisma: PrismaClient,
}

#[tokio::main]
async fn main() {
    let client = prisma::new_client().await;
    let prisma = client.unwrap();

    let shared_state = Arc::new(State { prisma });

    let app = Router::new()
        .route("/", get(root))
        .route("/register", post(register))
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
struct RegisterPayload {
    username: String,
    password: String,
}

async fn register(
    Extension(state): Extension<Arc<State>>,
    Json(payload): Json<RegisterPayload>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let created_user = prisma
        .user()
        .create(
            user::username::set(payload.username),
            user::password::set(payload.password),
            user::discriminator::set("0000".to_owned()),
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
            "discriminator": created_user_data.discriminator,
            "createdAt": created_user_data.created_at.to_string(),
        })),
    )
}
