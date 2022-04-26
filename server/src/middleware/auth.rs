use axum::{
    http::{self, Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use load_dotenv::load_dotenv;

use crate::Claims;

load_dotenv!();

pub async fn auth<B>(mut req: Request<B>, next: Next<B>) -> impl IntoResponse {
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
