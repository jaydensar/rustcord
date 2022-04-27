use std::sync::Arc;

use axum::{
    http::{self, Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use load_dotenv::load_dotenv;
use serde_json::json;

use crate::{prisma, Claims, State};

load_dotenv!();

pub async fn auth<B>(mut req: Request<B>, next: Next<B>) -> impl IntoResponse {
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let prisma = &req.extensions().get::<Arc<State>>().unwrap().prisma;

    let jwt_data = jsonwebtoken::decode::<Claims>(
        auth_header.unwrap_or("").replace("Bearer ", "").as_str(),
        &DecodingKey::from_secret(env!("JWT_SECRET").as_ref()),
        &Validation::new(Algorithm::HS256),
    );

    match jwt_data {
        Ok(data) => {
            let user_query = prisma
                .user()
                .find_unique(prisma::user::id::equals(data.claims.id.to_owned()))
                .with(prisma::user::memberships::fetch(vec![]))
                .exec()
                .await;

            if user_query.is_err() {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "User not found."})),
                )
                    .into_response();
            }

            let user_data = user_query.unwrap().unwrap();

            req.extensions_mut().insert(user_data);

            next.run(req).await
        }
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}
