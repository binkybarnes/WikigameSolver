use std::sync::Arc;

// src/routes/user.rs
use crate::models::ChangeUsernameRequest;
use crate::state::AppState;
use crate::util::json_response;
use axum::http::StatusCode;
use axum::{extract::State, response::IntoResponse, Extension, Json};
use rustrict::CensorStr;

pub async fn change_username_handler(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<ChangeUsernameRequest>,
) -> impl IntoResponse {
    let new_username = payload.username.trim();

    if new_username.is_empty() || new_username.chars().count() > 20 {
        return json_response(
            serde_json::json!({
                "success": false,
                "message": "Username must be 1–20 characters long"
            }),
            StatusCode::BAD_REQUEST,
        );
    }

    // Check if username contains inappropriate words
    if new_username.is_inappropriate() {
        return json_response(
            serde_json::json!({ "success": false, "message": "No bad words! 😡" }),
            StatusCode::BAD_REQUEST,
        );
    }

    // Check if username already exists
    let exists = sqlx::query_scalar!(
        "SELECT 1 FROM users WHERE username = ? LIMIT 1",
        new_username
    )
    .fetch_optional(&state.sqlite_pool)
    .await
    .unwrap();

    match exists {
        Some(_exists) => {
            return json_response(
                serde_json::json!({ "success": false, "message": "Username already taken" }),
                StatusCode::BAD_REQUEST,
            );
        }
        _ => {}
    }

    // Update the username in the database
    let result = sqlx::query!(
        "UPDATE users SET username = ? WHERE id = ?",
        new_username,
        user_id
    )
    .execute(&state.sqlite_pool)
    .await;

    match result {
        Ok(_) => json_response(
            serde_json::json!({ "success": true, "username": new_username }),
            StatusCode::OK,
        ),
        Err(err) => {
            eprintln!("Failed to update username: {:?}", err);
            json_response(
                serde_json::json!({ "success": false, "message": "Failed to update username" }),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}
