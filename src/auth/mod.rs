// src/auth/mod.rs

use crate::state::AppState;
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_cookies::Cookies;

pub mod google;

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub exp: usize,  // expiration timestamp
}

// Your jwt_middleware function
pub async fn jwt_middleware(
    State(state): State<Arc<AppState>>,
    cookies: Cookies,
    mut req: Request<Body>,
    next: Next,
) -> Response<Body> {
    // let pool: &Pool = &state.redis_pool;
    // let mut conn = pool.get().await.unwrap();

    // check for session cookie
    let session_cookie = cookies.get("jwt").map(|c| c.value().to_string());

    // if there is jwt cookie, add user_id to the request extention, if not dont do anything
    let maybe_user_id = if let Some(token) = session_cookie {
        match decode::<Claims>(
            &token,
            &DecodingKey::from_secret(state.env.jwt_secret.as_bytes()),
            &Validation::default(),
        ) {
            Ok(token_data) => Some(token_data.claims.sub),
            Err(_) => None,
        }
    } else {
        None // No cookie
    };

    // insert user_id into request extensions for handlers
    if let Some(user_id) = &maybe_user_id {
        req.extensions_mut().insert(user_id.clone());
    }

    // call next handler
    let response = next.run(req).await;

    response
}

// Your create_jwt function
pub fn create_jwt(user_id: &str, secret: &str) -> String {
    let exp = usize::MAX; // practically infinite expiry
    let claims = Claims {
        sub: user_id.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}
