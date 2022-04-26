mod prisma;

use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        Extension, Path,
    },
    http::{self, Request, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use hmac_sha256::Hash;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use load_dotenv::load_dotenv;
use prisma::PrismaClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;

load_dotenv!();

// todo add error handling

struct State {
    prisma: PrismaClient,
    tx: broadcast::Sender<SocketPayload>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Claims {
    username: String,
    id: String,
    exp: usize,
}

#[derive(Deserialize)]
struct AuthPayload {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct MessagePayload {
    content: String,
}

#[derive(Deserialize)]
struct ItemCreatePayload {
    name: String,
}

#[derive(Debug, Clone, Serialize)]
struct SocketMessagePayload {
    msg_type: String,
    content: String,
    author: User,
    channel_id: String,
    created_at: String,
    id: String,
}

#[derive(Debug, Clone, Serialize)]
struct User {
    id: String,
    username: String,
}

#[derive(Debug, Clone)]
enum SocketMessageType {
    NewMessage(SocketMessagePayload, String),
    GuildDataUpdate(String),
    UserGuildDataUpdate(String),
}

#[derive(Debug, Clone)]
struct SocketPayload {
    message: SocketMessageType,
}

#[tokio::main]
async fn main() {
    let client = prisma::new_client().await;
    let prisma = client.unwrap();

    let (tx, _rx) = broadcast::channel(100);

    let shared_state = Arc::new(State { prisma, tx });

    let router = Router::new()
        .route("/", get(root))
        .route("/register", post(register))
        .route("/login", post(login));

    let auth_router = Router::new()
        .route("/users/me", get(me))
        .route("/users/me/guilds", get(get_user_guilds))
        .route("/channels/:channel_id/messages", get(get_channel_messages))
        .route(
            "/channels/:channel_id/messages",
            post(post_channel_messages),
        )
        .route("/ws", get(socket_upgrade))
        .route("/guilds/create", post(create_guild))
        .route("/guilds/:guild_id/channels/create", post(create_channel))
        .route_layer(middleware::from_fn(auth));

    let app = Router::new()
        .merge(router)
        .merge(auth_router)
        .layer(Extension(shared_state));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("Listening on localhost:3000");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// jwt auth middleware
async fn auth<B>(mut req: Request<B>, next: Next<B>) -> impl IntoResponse {
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let jwt_data = jsonwebtoken::decode::<Claims>(
        auth_header.unwrap_or("").replace("Bearer ", "").as_str(),
        &DecodingKey::from_secret(env!("JWT_SECRET").as_ref()),
        &Validation::new(Algorithm::HS256),
    );

    match jwt_data {
        Ok(data) => {
            req.extensions_mut().insert(data.claims);
            next.run(req).await
        }
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn root() -> &'static str {
    "rc_api v0.1.0"
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
            exp: (chrono::offset::Utc::now().timestamp() + (15 * 60)) as usize,
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

async fn me(
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

async fn create_guild(
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

async fn create_channel(
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

async fn get_user_guilds(
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

async fn get_channel_messages(
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

async fn post_channel_messages(
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

async fn socket_upgrade(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<State>>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state, claims))
}

async fn websocket(mut socket: WebSocket, state: Arc<State>, claims: Claims) {
    let mut rx = state.tx.subscribe();

    let user_id = &claims.id;

    let mut user_data = state
        .prisma
        .user()
        .find_unique(prisma::user::id::equals(user_id.to_owned()))
        .with(prisma::user::WithParam::Memberships(vec![]))
        .exec()
        .await
        .unwrap()
        .unwrap();

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
