mod prisma;

use axum::{
    extract::Extension,
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router, TypedHeader,
};
use hmac_sha256::Hash;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use load_dotenv::load_dotenv;
use prisma::{user::username, *};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;

// on jwts:
// generate access token and refresh token on login
// access token expires in 15 minutes
// refresh token expires in 7 days, store refresh token
// when access token expires, client asks for new access token using the refresh token
// the refresh token expires on use, a new one is generated that lasts another 7 days

// todo add error handling

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
        .route("/me", get(me))
        .layer(Extension(shared_state));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("Server started.\nListening on {}", addr);

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
struct Claim {
    username: String,
    id: String,
    exp: usize,
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

    let access_token = jsonwebtoken::encode(
        &Header::default(),
        &Claim {
            username: user_data.username.to_owned(),
            id: user_data.id.to_owned(),
            exp: (chrono::offset::Utc::now().timestamp() + (15 * 60)) as usize,
        },
        &EncodingKey::from_secret(env!("JWT_TOKEN_SECRET").as_ref()),
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

async fn me(
    Extension(state): Extension<Arc<State>>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
) -> impl IntoResponse {
    let prisma = &state.prisma;

    let jwt_data = jsonwebtoken::decode::<Claim>(
        authorization.token(),
        &DecodingKey::from_secret(env!("JWT_TOKEN_SECRET").as_ref()),
        &Validation::new(jsonwebtoken::Algorithm::HS256),
    )
    .unwrap();

    let user_query = prisma
        .user()
        .find_unique(username::equals(jwt_data.claims.username))
        .with(user::WithParam::Memberships(vec![]))
        .exec()
        .await
        .unwrap()
        .unwrap();

    (
        StatusCode::CREATED,
        Json(json!({
            "id": user_query.id,
            "username": user_query.username,
            "createdAt": user_query.created_at.to_string(),
            "memberships": user_query.memberships().unwrap()
        })),
    )
}
