mod middleware;
mod prisma;
mod routes;

use axum::{
    extract::Extension,
    routing::{delete, get, post},
    Router,
};
use clap::Parser;
use log::info;
use prisma::PrismaClient;
use routes::socket::SocketPayload;
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    str::FromStr,
    sync::Arc,
};
use tokio::sync::broadcast;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, default_value = "::1")]
    host: String,

    #[clap(short, long, default_value = "3000")]
    port: u16,
}

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
pub struct User {
    id: String,
    username: String,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args = Args::parse();

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
        .route(
            "/guilds/:guild_id/invites/create",
            post(routes::guilds::create_invite),
        )
        .route("/guilds/join", post(routes::guilds::join_guild))
        .route_layer(axum::middleware::from_fn(middleware::auth::auth));

    let app = Router::new()
        .merge(router)
        .merge(authenticated_user_router)
        .layer(Extension(shared_state));

    let addr = SocketAddr::from((
        IpAddr::from_str(args.host.as_str()).unwrap_or(IpAddr::V6(Ipv6Addr::LOCALHOST)),
        args.port,
    ));

    info!("Listening on {}:{}", args.host, args.port);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root() -> &'static str {
    "rustcord api v0.1.0"
}
