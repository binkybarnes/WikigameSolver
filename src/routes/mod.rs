// src/routes/mod.rs

use crate::auth::jwt_middleware;
use crate::state::AppState;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_cookies::CookieManagerLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;

// Import your handlers
mod auth;
mod search;
mod user;

use auth::{google_auth_login_handler, logout_handler, me_handler};
use search::search_handler;
use user::change_username_handler;

pub fn create_router(state: Arc<AppState>, cors: CorsLayer) -> Router {
    Router::new()
        .route("/search", post(search_handler))
        .route("/me", get(me_handler))
        .route("/auth/google", post(google_auth_login_handler))
        .route("/auth/logout", post(logout_handler))
        .route("/user/change-username", post(change_username_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            jwt_middleware,
        ))
        .with_state(state)
        .layer(CookieManagerLayer::new())
        .layer(CompressionLayer::new())
        // .layer(GovernorLayer::new(governor_conf)) // You can pass governor_conf in if needed
        .layer(cors)
}
