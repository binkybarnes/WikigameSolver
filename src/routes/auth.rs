use std::sync::Arc;

// src/routes/auth.rs
use crate::models::{create_guest_account, Provider, UserInfo};
use crate::state::AppState;
use crate::util::json_response;
use crate::{
    auth::{
        create_jwt,
        google::{verify_google_token, AuthRequest},
        Claims,
    },
    leaderboard::update_username_in_redis,
};
use axum::{
    body::Body,
    extract::State,
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::Utc;
use jsonwebtoken::{decode, DecodingKey, Validation};
use reqwest::StatusCode;
use serde_json::json;
use tower_cookies::{Cookie, Cookies};
use uuid::Uuid;

pub async fn me_handler(State(state): State<Arc<AppState>>, cookies: Cookies) -> impl IntoResponse {
    if let Some(cookie) = cookies.get("jwt") {
        let token = cookie.value().to_string();
        match decode::<Claims>(
            &token,
            &DecodingKey::from_secret(state.env.jwt_secret.as_bytes()),
            &Validation::default(),
        ) {
            Ok(token_data) => {
                let user_id = token_data.claims.sub;
                if let Ok(row) =
                    sqlx::query!("SELECT username, provider FROM users WHERE id = ?", user_id)
                        .fetch_one(&state.sqlite_pool)
                        .await
                {
                    let username: String = row.username;
                    let provider: Provider = row.provider.into();

                    let body = serde_json::to_string(&UserInfo {
                        user_id,
                        username,
                        provider,
                    })
                    .unwrap();

                    return Response::builder()
                        .status(StatusCode::CREATED)
                        .header("Content-Type", "application/json")
                        .header("Cache-Control", "no-store") // example header
                        .body(Body::from(body))
                        .unwrap();
                } else {
                    // Token refers to missing user -> fallthrough to create guest
                    tracing::warn!("JWT referenced missing user: {}", user_id);
                }
            }
            Err(_) => {
                // invalid token -> we'll create a guest below
            }
        }
    }

    // No valid token -> create guest user, set cookie, return info
    let (guest_id, username): (String, String) =
        match create_guest_account(&state.sqlite_pool).await {
            Ok((id, name)) => (id, name),
            Err(e) => {
                tracing::error!("failed to create guest: {:?}", e);
                return json_response(
                    json!({"error":"internal"}),
                    StatusCode::INTERNAL_SERVER_ERROR.into(),
                );
            }
        };

    // create jwt and set cookie
    let token = create_jwt(&guest_id, &state.env.jwt_secret);
    cookies.add(
        Cookie::build(("jwt", token))
            .path("/")
            .http_only(true)
            .secure(state.env.is_production) // secure in prod
            .same_site(tower_cookies::cookie::SameSite::Lax)
            .into(),
    );

    let body = serde_json::to_string(&UserInfo {
        user_id: guest_id,
        username: username,
        provider: Provider::Guest,
    })
    .unwrap();

    Response::builder()
        .status(StatusCode::CREATED)
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-store") // example header
        .body(Body::from(body))
        .unwrap()
}

pub async fn google_auth_login_handler(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<String>,
    cookies: tower_cookies::Cookies,
    Json(payload): Json<AuthRequest>,
) -> impl IntoResponse {
    // 1) config
    let client_id = &state.env.google_client_id;

    // 2) --- Verification Step ---
    // We assign the result of theA match to the `claims` variable.
    let claims = match verify_google_token(&payload.token, &client_id).await {
        Ok(verified_claims) => {
            // Token is valid, let the handler continue with the claims.
            verified_claims
        }
        Err(e) => {
            // Token is invalid. Log the specific error and return early.
            eprintln!("Token verification failed: {}", e);
            return json_response(
                json!({"error": "Invalid or expired token", "details": e }),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    // Token is valid! We can now trust the claims within.
    let google_id = claims.sub;
    let email = claims.email;
    let name = claims.name.unwrap_or_default();

    tracing::info!("Verified Google user: {} ({})", name, email);

    // 3) start transaction
    let mut tx = match state.sqlite_pool.begin().await {
        Ok(tx) => tx,
        Err(err) => {
            return json_response(
                json!({ "error": err.to_string() }),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    // 4) fetch current (requesting) user row to see provider
    let current_user_row = match sqlx::query!(
        "SELECT id, provider, username FROM users WHERE id = ?",
        user_id
    )
    .fetch_optional(&mut *tx)
    .await
    {
        Ok(opt) => opt.unwrap(), // hopefully the user sending request has a user id
        Err(err) => {
            eprintln!("DB error fetching current user: {}", err);
            return json_response(
                json!({ "error": err.to_string() }),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    let current_provider = current_user_row.provider;
    let current_username = current_user_row.username;
    let final_user_id: String;
    let mut final_username: Option<String> = None;

    println!("google_id {}", google_id);
    // Helper: check if google user already exists
    let existing_google_user = sqlx::query!(
        "SELECT id, username FROM users WHERE provider = 'google' AND provider_id = ?",
        google_id
    )
    .fetch_optional(&mut *tx)
    .await;

    let mut first_time = false;

    if current_provider == "guest" {
        // --- The normal/expected flow: requester is a guest -> either merge into existing google or upgrade guest to google
        let guest_user_id = &user_id;

        match existing_google_user {
            Ok(Some(row)) => {
                // Merge guest -> existing google account
                let google_user_id = row.id.unwrap();

                if let Err(err) = sqlx::query!(
                    "UPDATE searches SET user_id = ? WHERE user_id = ?",
                    google_user_id,
                    guest_user_id
                )
                .execute(&mut *tx)
                .await
                {
                    return json_response(
                        json!({ "error": err.to_string() }),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    );
                }

                if let Err(err) = sqlx::query!(
                    "DELETE FROM users WHERE id = ? AND provider = 'guest'",
                    guest_user_id
                )
                .execute(&mut *tx)
                .await
                {
                    return json_response(
                        json!({ "error": err.to_string() }),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    );
                }

                final_user_id = google_user_id;
                final_username = Some(row.username);
                tracing::info!("Merged guest {} -> google {}", guest_user_id, final_user_id);
            }
            Ok(None) => {
                // Upgrade guest -> google (set provider and provider_id, update username)
                let guest_user_id = user_id;
                if let Err(err) = sqlx::query!(
                    "UPDATE users SET provider = 'google', provider_id = ? WHERE id = ?",
                    google_id,
                    // name,
                    guest_user_id
                )
                .execute(&mut *tx)
                .await
                {
                    return json_response(
                        json!({ "error": err.to_string() }),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    );
                }

                final_user_id = guest_user_id;
                // final_username = Some(name);
                first_time = true;
                tracing::info!(
                    "Upgraded guest {} -> google (provider_id set)",
                    final_user_id
                );
            }
            Err(err) => {
                return json_response(
                    json!({ "error": err.to_string() }),
                    StatusCode::INTERNAL_SERVER_ERROR,
                );
            }
        }
    } else {
        // --- The "odd case": requester is NOT a guest. DO NOT merge or upgrade the requester.
        // If a google account already exists with this google_id => log into that account
        // else => create a new google user row (separate account) and log into it.
        match existing_google_user {
            Ok(Some(row)) => {
                final_user_id = row.id.unwrap();
                tracing::info!(
                    "Requester was non-guest; google account exists. Logging into {}",
                    final_user_id
                );
            }
            Ok(None) => {
                // Create a new user row for this google account (do NOT touch the requester)
                let new_id = Uuid::new_v4().to_string();
                let created_at = Utc::now().to_rfc3339();

                if let Err(err) = sqlx::query!(
                "INSERT INTO users (id, provider, provider_id, username, created_at) VALUES (?, 'google', ?, ?, ?)",
                new_id,
                google_id,
                name,
                created_at
                    )
                .execute(&mut *tx)
                .await
                {
                    return json_response(
                        json!({ "error": err.to_string() }),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    );
                }

                final_user_id = new_id;
                first_time = true;
                tracing::info!(
                    "Requester was non-guest; created new google user {} (no merge/upgrade)",
                    final_user_id
                );
            }
            Err(err) => {
                return json_response(
                    json!({ "error": err.to_string() }),
                    StatusCode::INTERNAL_SERVER_ERROR,
                );
            }
        }
    }

    // Commit transaction
    if let Err(err) = tx.commit().await {
        return json_response(
            json!({ "error": err.to_string() }),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }
    if let Some(final_username) = &final_username {
        if let Err(err) =
            update_username_in_redis(&state.redis_pool, &current_username, final_username).await
        {
            tracing::error!("Failed to update Redis leaderboard usernames: {}", err);
        }
    }
    // 3. Create JWT for this user
    let token = create_jwt(&final_user_id, &state.env.jwt_secret);

    // 4. Set JWT cookie (persistent ~100 years)
    cookies.add(
        Cookie::build(("jwt", token))
            .path("/")
            .secure(state.env.is_production) // ⚠️ should be true in production (HTTPS)
            .http_only(true)
            .max_age(tower_cookies::cookie::time::Duration::days(36500))
            .into(),
    );

    json_response(
        json!({ "user_id": final_user_id, "first_time": first_time }),
        StatusCode::OK,
    )
}

pub async fn logout_handler(
    State(state): State<Arc<AppState>>,
    cookies: Cookies,
) -> impl IntoResponse {
    // Clear the JWT cookie
    cookies.add(
        Cookie::build(("jwt", ""))
            .path("/")
            .http_only(true)
            .secure(state.env.is_production) // ⚠️ set true in production
            .max_age(tower_cookies::cookie::time::Duration::seconds(0)) // expire immediately
            .into(),
    );

    json_response(
        serde_json::json!({
            "success": true,
            "message": "Logged out successfully"
        }),
        StatusCode::OK,
    )
}
