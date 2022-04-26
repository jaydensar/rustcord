mod middleware;
mod prisma;
mod routes;

use axum::{
    extract::Extension,
    routing::{delete, get, post},
    Router,
};
use prisma::PrismaClient;
use routes::socket::SocketPayload;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;

pub struct State {
    prisma: PrismaClient,
    tx: broadcast::Sender<SocketPayload>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Claims {
    username: String,
    id: String,
    exp: usize,
}

#[derive(Debug, Clone, Serialize)]
struct User {
    id: String,
    username: String,
}

#[tokio::main]
async fn main() {
    let client = prisma::new_client().await;
    let prisma = client.unwrap();

    let (tx, _rx) = broadcast::channel(100);

    let shared_state = Arc::new(State { prisma, tx });

    let router = Router::new()
        .route("/", get(root))
        .route("/register", post(routes::auth::register))
        .route("/login", post(routes::auth::login));

    let authenticated_user_router = Router::new()
        .route("/ws", get(routes::socket::upgrade))
        .route("/users/me", get(routes::users::me))
        .route("/users/me/guilds", get(routes::users::get_user_guilds))
        .route(
            "/channels/:channel_id/messages",
            get(routes::channels::get_channel_messages),
        )
        .route(
            "/channels/:channel_id/messages",
            post(routes::channels::post_channel_messages),
        )
        .route("/guilds/create", post(routes::guilds::create_guild))
        .route(
            "/guilds/:guild_id/delete",
            delete(routes::guilds::delete_guild),
        )
        .route(
            "/guilds/:guild_id/channels/create",
            post(routes::guilds::create_channel),
        )
        .route_layer(axum::middleware::from_fn(middleware::auth::auth));

    let app = Router::new()
        .merge(router)
        .merge(authenticated_user_router)
        .layer(Extension(shared_state));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("Listening on localhost:3000");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root() -> &'static str {
    "rustcord api v0.1.0"
}
