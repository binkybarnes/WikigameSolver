// Declare all your new modules
mod auth;
mod config;
mod leaderboard;
mod models;
mod state;
mod util;

// Your other existing modules
mod builders;
mod graph;
mod mmap_structs;
mod parsers;
mod routes;
mod search; // This is important!

use core::num;
use std::env;
use std::hash::Hash;
use std::net::SocketAddr;
use std::time::Duration;
use std::time::Instant;

use crate::builders::*;
use crate::graph::*;
use crate::mmap_structs::*;

use anyhow::bail;
use axum::body::Body;
use axum::extract::Query;
use axum::http;
use axum::http::header::SET_COOKIE;
use axum::http::header::USER_AGENT;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Extension;
use chrono::Utc;
use clap::Parser;
use deadpool_redis::PoolError;
use deadpool_redis::{Config as RedisConfig, Pool};

// todo:
// see how much memory the pagelinks hashmap uses (use the rust memory cli tool?)
// try out csr to see if its less memory (including the 2 id maps)
// see which is faster for bfs, csr or hashmap adjacency list
//   check if memory or cpu is bottleneck
// check one direction bfs speed, then make a incoming links graph if memory permits, for bidirectional bfs
// parallel bfs?

// replaced bincode serialization with rkyv see if its faster

// reordering for locality (for csr):
//   for csr RCM (Reverse Cuthill-McKee), putting similar pages together
//   or reordering with community detection (louvain, Label Propagation, Girvan–Newman, Infomap, etc)
//   or graph partitioning (for parallel processing or community detection?) (METIS, KaHIP)

// maybe make title to id titles lowercase

#[derive(Parser)]
#[command(name = "wikirace")]
#[command(about = "Find shortest paths between Wikipedia pages", long_about = None)]
struct Args {
    /// Rebuild the memory-mapped files
    #[arg(long)]
    rebuild: bool,

    /// Port for the API server
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}
use axum::{extract::State, routing::post, Router};

use rand::distr::Alphanumeric;
use rand::Rng;
use redis::AsyncCommands;
use redis::RedisResult;
use rustc_hash::FxBuildHasher;
use rustc_hash::FxHashMap;
use rustc_hash::FxHasher;
use serde_json::to_vec;
use sqlx::sqlite;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    title_to_dense_id: Arc<TitleToDenseIdMmap>,
    dense_id_to_title: Arc<DenseIdToTitleMmap>,
    orig_to_dense_id: Arc<OrigToDenseIdMmap>,
    dense_id_to_orig: Arc<DenseIdToOrigMmap>,
    redirects_passed: Arc<RedirectsPassedMmap>,
    redirect_targets_dense: Arc<RedirectTargetsDenseMmap>,
    csr_graph: Arc<CsrGraphMmap>,
    redis_pool: deadpool_redis::Pool,
    sqlite_pool: sqlx::SqlitePool,
    env: EnvironmentVariables,
}

use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::hash::Hasher;
use tower_http::compression::CompressionLayer;

#[derive(Debug, Deserialize)]
pub struct PathRequest {
    #[serde(default)]
    start: Option<String>,
    #[serde(default)]
    start_id: Option<u32>,
    #[serde(default)]
    end: Option<String>,
    #[serde(default)]
    end_id: Option<u32>,
    #[serde(default)]
    output_as_ids: bool,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PathNode {
    Title(String),
    Id(u32),
}

#[derive(Debug, Serialize)]
pub struct PathResponse {
    elapsed_s: f64,
    paths: Vec<Vec<PathNode>>,
    leaderboard_longest_rank: Option<u32>,
    leaderboard_most_rank: Option<u32>,
}

use axum::{
    http::{
        header::{ACCEPT, CACHE_CONTROL, CONTENT_TYPE, COOKIE, ETAG, IF_NONE_MATCH},
        Request,
    },
    response::IntoResponse,
};

fn json_response(body: serde_json::Value, status: StatusCode) -> Response {
    let body_bytes = serde_json::to_vec(&body).unwrap();
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(body_bytes))
        .unwrap()
}

use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

// #[axum::debug_handler]
// pub async fn search_handler(
//     State(state): State<Arc<AppState>>,
//     // headers: HeaderMap,
//     // Extension(user_id): Extension<String>,
//     Json(req): Json<PathRequest>,
// ) -> impl IntoResponse {
//     let start_req = Instant::now();
// }
#[axum::debug_handler]
pub async fn search_handler(
    State(state): State<Arc<AppState>>,
    // headers: HeaderMap,
    Extension(user_id): Extension<String>,
    Json(req): Json<PathRequest>,
) -> impl IntoResponse {
    let start_req = Instant::now();

    // --- Resolve start ---
    let start_id = match (&req.start, &req.start_id) {
        (Some(title), None) => match state.title_to_dense_id.get(title) {
            Some(id) => id,
            None => {
                tracing::error!("bad request, reason=Start title not found: {}", title);
                return json_response(
                    json!({"error": format!("Start title '{}' not found. This article might be too new, or check capitalization", title)}),
                    StatusCode::NOT_FOUND,
                );
            }
        },
        (None, Some(orig_id)) => match state.orig_to_dense_id.get(*orig_id) {
            Some(id) => id,
            None => {
                return json_response(
                    json!({"error": format!("Start original ID '{}' not found", orig_id)}),
                    StatusCode::NOT_FOUND,
                )
            }
        },
        _ => {
            return json_response(
                json!({"error": "Exactly one of start or start_id must be provided"}),
                StatusCode::BAD_REQUEST,
            )
        }
    };

    // --- Resolve end ---
    let goal_id = match (&req.end, &req.end_id) {
        (Some(title), None) => match state.title_to_dense_id.get(title) {
            Some(id) => id,
            None => {
                tracing::error!("bad request, reason=end title not found: {}", title);

                return json_response(
                    json!({"error": format!("End title '{}' not found. Check capitalization", title)}),
                    StatusCode::NOT_FOUND,
                );
            }
        },
        (None, Some(orig_id)) => match state.orig_to_dense_id.get(*orig_id) {
            Some(id) => id,
            None => {
                return json_response(
                    json!({"error": format!("End original ID '{}' not found", orig_id)}),
                    StatusCode::NOT_FOUND,
                )
            }
        },
        _ => {
            return json_response(
                json!({"error": "Exactly one of end or end_id must be provided"}),
                StatusCode::BAD_REQUEST,
            )
        }
    };

    // --- Resolve redirects ---
    let start_id = match state.redirect_targets_dense.get(start_id) {
        u32::MAX => start_id,
        redirect => redirect,
    };
    let goal_id = match state.redirect_targets_dense.get(goal_id) {
        u32::MAX => goal_id,
        redirect => redirect,
    };

    // let mut hasher = FxHasher::default();
    // start_id.hash(&mut hasher);
    // goal_id.hash(&mut hasher);
    // let etag = format!("{:x}", hasher.finish());
    // let etag = format!("\"{}-{}-{}\"", start_id, goal_id, req.output_as_ids);

    // println!("ETag: {}", etag);

    // if let Some(if_none_match) = headers.get(IF_NONE_MATCH) {
    //     if if_none_match.to_str().ok() == Some(&etag) {
    //         println!("Response time: {:.2?}\n", start_req.elapsed());
    //         return Response::builder()
    //             .status(304)
    //             .header(ETAG, &etag)
    //             .header(CACHE_CONTROL, "public, max-age=31536000, immutable")
    //             .body(Body::empty())
    //             .unwrap();
    //     }
    // }

    // --- Run BFS ---
    let start_bfs = Instant::now();
    let mut node_count = 0;
    let result = search::bi_bfs_csr(
        &*state.csr_graph,
        start_id,
        goal_id,
        50,
        &state.redirects_passed,
        &mut node_count,
    )
    .unwrap_or_default();
    let elapsed_s = start_bfs.elapsed().as_secs_f64();

    // --- Convert paths ---
    let paths: Vec<Vec<PathNode>> = result
        .into_iter()
        .map(|path| {
            path.into_iter()
                .map(|dense_id| {
                    if req.output_as_ids {
                        PathNode::Id(state.dense_id_to_orig.get(dense_id))
                    } else {
                        PathNode::Title(state.dense_id_to_title.get(dense_id).to_string())
                    }
                })
                .collect()
        })
        .collect();

    let sql_time = Instant::now();
    let num_paths = paths.len() as u32;
    let path_length = if let Some(first_path) = paths.first() {
        first_path.len() as u32
    } else {
        0
    };

    if num_paths == 0 {
        println!("no path found");
    }
    let search_id = Uuid::new_v4().to_string();
    let start_id_orig = state.dense_id_to_orig.get(start_id);
    let goal_id_orig = state.dense_id_to_orig.get(goal_id);
    let created_at = Utc::now().format("%Y-%m-%dT%H:%M%z").to_string();

    match sqlx::query!(
        r#"
        INSERT INTO searches
        (id, user_id, start_id, end_id, elapsed_s, nodes_visited, path_length, num_paths, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
        search_id,
        user_id,
        start_id_orig,
        goal_id_orig,
        elapsed_s,
        node_count,
        path_length,
        num_paths,
        created_at
    )
    .execute(&state.sqlite_pool)
    .await{
        Ok(r) => {
            tracing::debug!("inserted search rows_affected={}", r.rows_affected());
        }
        Err(e) => {
            tracing::error!("insert search failed: {:?}", e);
        }
    };

    let leaderboard_longest_rank = try_add_to_leaderboard(
        &state.sqlite_pool,
        &state.redis_pool,
        "longest",
        start_id_orig,
        goal_id_orig,
        path_length,
        &search_id,
        state.env.leaderboard_limit,
    )
    .await;
    let leaderboard_most_rank = try_add_to_leaderboard(
        &state.sqlite_pool,
        &state.redis_pool,
        "most",
        start_id_orig,
        goal_id_orig,
        num_paths,
        &search_id,
        state.env.leaderboard_limit,
    )
    .await;

    println!("db stuff took {:?}", sql_time.elapsed());
    let response = PathResponse {
        elapsed_s,
        paths,
        leaderboard_longest_rank,
        leaderboard_most_rank,
    };

    // Optional: log size
    let body = serde_json::to_vec(&response).unwrap();

    println!("Response size: {} bytes", body.len());

    println!("Response time: {:.2?}\n", start_req.elapsed());

    // Build response with headers
    Response::builder()
        .header("Content-Type", "application/json")
        // .header("ETag", etag)
        .header("Cache-Control", "public, max-age=31536000, immutable") // ~1 year
        .body(Body::from(body))
        .unwrap()

    // Json(json!(response))
}

pub async fn try_add_to_leaderboard(
    sqlite_pool: &sqlx::SqlitePool,
    redis_pool: &deadpool_redis::Pool,
    leaderboard_type: &str,
    start_id: u32,
    end_id: u32,
    score: u32,
    search_id: &str,
    top_n: u32,
) -> Option<u32> {
    let claim = sqlx::query!(
        r#"
        INSERT OR IGNORE INTO claimed_paths (start_id, end_id, leaderboard, search_id)
        VALUES (?1, ?2, ?3, ?4)
        "#,
        start_id,
        end_id,
        leaderboard_type,
        search_id,
    )
    .execute(sqlite_pool)
    .await
    .unwrap();

    if claim.rows_affected() != 1 {
        return None;
    }

    let leaderboard_key = &format!("leaderboard:{}", leaderboard_type);

    let mut conn = redis_pool.get().await.unwrap();

    // 1️⃣ Check current size
    let size: u32 = conn.zcard(leaderboard_key).await.unwrap();
    let path_value = format!("{}|{}", start_id, end_id);

    if size < top_n {
        // Leaderboard not full → insert
        conn.zadd::<&str, u32, &str, ()>(leaderboard_key, &path_value, score)
            .await
            .unwrap();
        tracing::info!("Added path {} to {}", path_value, leaderboard_key);
    } else {
        // Leaderboard full → check lowest score
        let lowest: Option<(String, u32)> = conn
            .zrange_withscores::<&str, Vec<(String, u32)>>(leaderboard_key, 0, 0)
            .await
            .unwrap()
            .into_iter()
            .next();

        if let Some((_, lowest_score)) = lowest {
            if score > lowest_score {
                // New search qualifies → insert
                conn.zadd::<&str, u32, &str, ()>(leaderboard_key, &path_value, score)
                    .await
                    .unwrap();
                tracing::info!("Added path {} to {}", path_value, leaderboard_key);
                // Trim to top N
                let start = 0;
                let stop = -(top_n as isize) - 1;
                let _: () = conn
                    .zremrangebyrank(leaderboard_key, start, stop)
                    .await
                    .unwrap();
            } else {
                // Does not qualify
                return None;
            }
        }
    }

    // 2️⃣ Return the new rank (0-based)
    let rank: Option<u32> = conn.zrevrank(leaderboard_key, path_value).await.unwrap();
    rank
}

use dotenv::dotenv;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use tower_cookies::{Cookie, CookieManagerLayer, Cookies};

use http::Method;

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String, // user_id
    exp: usize,  // expiration timestamp
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Provider {
    Guest,
    Google,
}
impl From<String> for Provider {
    fn from(s: String) -> Self {
        match s.as_str() {
            "google" => Provider::Google,
            _ => Provider::Guest,
        }
    }
}
impl From<Provider> for String {
    fn from(p: Provider) -> Self {
        match p {
            Provider::Google => "google".to_string(),
            Provider::Guest => "guest".to_string(),
        }
    }
}

#[derive(Serialize)]
struct UserInfo {
    user_id: String,
    username: String,
    provider: Provider,
}

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
            .secure(false) // secure in prod
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

// Create a new guest user_id
pub async fn create_guest_account(
    sqlite_pool: &sqlx::SqlitePool,
) -> anyhow::Result<(String, String)> {
    let guest_id = Uuid::new_v4().to_string();
    let provider = "guest";
    let provider_id = guest_id.clone();
    let username = format!("Guest{}", &guest_id[..5]);
    let created_at = Utc::now().format("%Y-%m-%dT%H:%M%z").to_string();

    sqlx::query!(
        r#"
        INSERT INTO users (id, provider, provider_id, username, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        guest_id,
        provider,
        provider_id,
        username,
        created_at,
    )
    .execute(sqlite_pool)
    .await?;

    tracing::info!(
        "Created guest account: {} with username {}",
        guest_id,
        username
    );

    Ok((guest_id, username))
}

// // Create a new guest user_id
// pub async fn create_guest_account(
//     sqlite_pool: &sqlx::SqlitePool,
// ) -> anyhow::Result<(String, String)> {

// }

// Create JWT from user_id
fn create_jwt(user_id: &str, secret: &str) -> String {
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

// This struct captures the incoming request from the React frontend.
// It remains the same.
#[derive(Deserialize)]
struct AuthRequest {
    token: String,
}

// This is the user profile we will send back to the frontend.
// It could also include your application's own session token.
#[derive(Serialize)]
struct UserProfile {
    id: String,        // This would be your database ID
    google_id: String, // The 'sub' field from Google
    email: String,
    name: String,
}

use jsonwebtoken::{decode_header, Algorithm};
use reqwest::Client;

// Struct to hold the claims from the Google ID token
#[derive(Debug, Clone, Deserialize)]
pub struct GoogleClaims {
    pub iss: String, // Issuer
    pub aud: String, // Audience (your client ID)
    pub exp: usize,  // Expiration time
    pub sub: String, // User's unique Google ID
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
}

// Structs to deserialize Google's public keys (JWKS)
#[derive(Debug, Deserialize)]
struct Jwk {
    keys: Vec<JsonWebKey>,
}

#[derive(Debug, Deserialize)]
struct JsonWebKey {
    kid: String, // Key ID
    alg: String, // Algorithm (e.g., "RS256")
    n: String,   // Modulus
    e: String,   // Exponent
}

/// Verifies a Google ID token by fetching public keys and checking claims.
/// This follows the manual verification steps outlined by Google.
pub async fn verify_google_token(token_str: &str, client_id: &str) -> Result<GoogleClaims, String> {
    // 1. Fetch Google's public keys
    // In a real app, you should cache these keys based on the Cache-Control header.
    let client = Client::new();
    let jwks_url = "https://www.googleapis.com/oauth2/v3/certs";
    let jwks = client
        .get(jwks_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch JWKS: {}", e))?
        .json::<Jwk>()
        .await
        .map_err(|e| format!("Failed to parse JWKS: {}", e))?;

    // 2. Decode the token's header to find the Key ID (`kid`)
    let header = decode_header(token_str).map_err(|e| format!("Invalid token header: {}", e))?;

    let kid = header
        .kid
        .ok_or_else(|| "Token header missing 'kid'".to_string())?;

    // 3. Find the matching public key from the JWKS
    let matching_key = jwks
        .keys
        .iter()
        .find(|key| key.kid == kid)
        .ok_or_else(|| "No matching public key found".to_string())?;

    // 4. Create a decoding key from the public key's components (n, e)
    let decoding_key = DecodingKey::from_rsa_components(&matching_key.n, &matching_key.e)
        .map_err(|e| format!("Failed to create decoding key: {}", e))?;

    // 5. Set up validation rules
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[client_id]); // Check 'aud'
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]); // Check 'iss'

    // 6. Decode and validate the token
    // This function checks the signature, expiration, issuer, and audience.
    let token_data = decode::<GoogleClaims>(token_str, &decoding_key, &validation)
        .map_err(|e| format!("Token validation failed: {}", e))?;

    Ok(token_data.claims)
}

/// The primary authentication handler, now returning `impl IntoResponse`.
async fn google_auth_login_handler(
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
    let current_user_row =
        match sqlx::query!("SELECT id, provider FROM users WHERE id = ?", user_id)
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
    let final_user_id: String;

    println!("google_id {}", google_id);
    // Helper: check if google user already exists
    let existing_google_user = sqlx::query!(
        "SELECT id, username FROM users WHERE provider = 'google' AND provider_id = ?",
        google_id
    )
    .fetch_optional(&mut *tx)
    .await;

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
                tracing::info!("Merged guest {} -> google {}", guest_user_id, final_user_id);
            }
            Ok(None) => {
                // Upgrade guest -> google (set provider and provider_id, update username)
                let guest_user_id = user_id;
                if let Err(err) = sqlx::query!(
                "UPDATE users SET provider = 'google', provider_id = ?, username = ? WHERE id = ?",
                google_id,
                name,
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

    // 3. Create JWT for this user
    let token = create_jwt(&final_user_id, &state.env.jwt_secret);

    // 4. Set JWT cookie (persistent ~100 years)
    cookies.add(
        Cookie::build(("jwt", token))
            .path("/")
            .secure(false) // ⚠️ should be true in production (HTTPS)
            .http_only(true)
            .max_age(tower_cookies::cookie::time::Duration::days(36500))
            .into(),
    );

    json_response(json!({ "user_id": final_user_id}), StatusCode::OK)
}

async fn logout_handler(cookies: Cookies) -> impl IntoResponse {
    // Clear the JWT cookie
    cookies.add(
        Cookie::build(("jwt", ""))
            .path("/")
            .http_only(true)
            .secure(false) // ⚠️ set true in production
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

#[derive(Deserialize)]
pub struct ChangeUsernameRequest {
    pub username: String,
}
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

use sqlx::Row;

pub async fn populate_leaderboard(
    sqlite_pool: &sqlx::SqlitePool,
    redis_pool: &deadpool_redis::Pool,
    leaderboard: &str,   // "most or longest"
    metric_column: &str, // e.g., "path_length" or "num_paths"
    top_n: u32,
) -> anyhow::Result<()> {
    let mut redis_conn = redis_pool.get().await?;

    // 1️⃣ Clear the leaderboard
    let _: () = redis_conn.del(leaderboard).await?;

    // 2️⃣ Fetch top N searches from SQLite
    let query = format!(
        "SELECT cp.start_id, cp.end_id, cp.search_id, s.{}
        FROM claimed_paths cp
        JOIN searches s ON s.id = cp.search_id
        WHERE cp.leaderboard = ?
        ORDER BY s.{} DESC
        LIMIT ?;
        ",
        metric_column, metric_column
    );

    let rows = sqlx::query(&query)
        .bind(leaderboard)
        .bind(top_n)
        .fetch_all(sqlite_pool)
        .await?;

    // 3️⃣ Insert into Redis leaderboard
    let leaderboard_key = format!("leaderboard:{}", leaderboard);
    for row in rows {
        let start_id: u32 = row.get("start_id");
        let end_id: u32 = row.get("end_id");
        let score: u32 = row.try_get(metric_column)?; // path_length or num_paths

        let path_value = format!("{}|{}", start_id, end_id);

        let _: () = redis_conn.zadd(&leaderboard_key, path_value, score).await?;
    }

    Ok(())
}

fn init_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    // Make logs directory
    std::fs::create_dir_all("logs").ok();

    // File appender, daily rotation
    let file_appender = rolling::daily("logs", "error.log");
    let (file_writer, file_guard) = non_blocking(file_appender);

    // Layer that writes only ERROR+ to file
    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_filter(LevelFilter::ERROR);

    // Layer that writes INFO+ to stdout
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_filter(LevelFilter::INFO);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stdout_layer)
        .init();

    file_guard
}

#[derive(Clone, Debug)]
pub struct EnvironmentVariables {
    pub jwt_secret: String,
    pub database_url: String,
    pub leaderboard_limit: u32,
    pub google_client_id: String,
}

impl EnvironmentVariables {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenv().ok();

        let jwt_secret = match env::var("JWT_SECRET") {
            Ok(val) => val,
            Err(_) => bail!("Missing JWT_SECRET"),
        };

        let database_url = match env::var("DATABASE_URL") {
            Ok(val) => val,
            Err(_) => bail!("Missing DATABASE_URL"),
        };

        let leaderboard_limit = match env::var("LEADERBOARD_LIMIT") {
            Ok(val) => val.parse::<u32>()?,
            Err(_) => bail!("Missing LEADERBOARD_LIMIT"),
        };

        let google_client_id = match env::var("GOOGLE_CLIENT_ID") {
            Ok(val) => val,
            Err(_) => bail!("Missing GOOGLE_CLIENT_ID"),
        };

        Ok(Self {
            jwt_secret,
            database_url,
            leaderboard_limit,
            google_client_id,
        })
    }
}

use tracing_appender::non_blocking;
use tracing_appender::rolling;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let now = Instant::now();

    let args = Args::parse();

    let guard = init_tracing();
    tracing::error!("this is a test error");

    if args.rebuild {
        println!("Rebuilding structures...");

        // build and save normal structures
        build_and_save_page_maps_dense()?;
        // ↓
        build_and_save_linktargets_dense()?;
        build_and_save_redirect_targets_dense()?;
        // ↓
        build_and_save_pagelinks_adj_list()?;
        // ↓
        build_and_save_pagelinks_csr()?;

        // build and save mmap structures
        build_and_save_title_to_dense_id_mmap()?;
        build_and_save_dense_id_to_title_mmap()?;
        build_and_save_orig_to_dense_id_mmap()?;
        build_and_save_dense_id_to_orig_mmap()?;
        build_and_save_pagelinks_csr()?;
        build_and_save_redirects_passed_mmap()?;
        build_and_save_redirect_targets_dense_mmap()?;
    }

    // load normal structures
    // let csr_graph: CsrGraph = util::load_from_file("data/pagelinks_csr.bin")?;

    // // load mmap structures
    // let title_to_dense_id_mmap: TitleToDenseIdMmap = load_title_to_dense_id_mmap()?;
    // let dense_id_to_title_mmap: DenseIdToTitleMmap = load_dense_id_to_title_mmap()?;
    // let orig_to_dense_id: OrigToDenseIdMmap = load_orig_to_dense_id_mmap()?;
    // let dense_id_to_orig: DenseIdToOrigMmap = load_dense_id_to_orig_mmap()?;
    // let redirects_passed_mmap: RedirectsPassedMmap = load_redirects_passed_mmap()?;
    // let redirect_targets_dense_mmap: RedirectTargetsDenseMmap = load_redirect_targets_dense_mmap()?;
    // let csr_graph_mmap: CsrGraphMmap = load_csr_graph_mmap()?;

    // search::bfs_interactive_session(
    //     &title_to_dense_id_mmap,
    //     &dense_id_to_title_mmap,
    //     &csr_graph_mmap,
    //     &redirect_targets_dense_mmap,
    //     &redirects_passed_mmap,
    // );

    let env = EnvironmentVariables::from_env()?;

    let redis_cfg = RedisConfig::from_url("redis://127.0.0.1/");
    let redis_pool = redis_cfg
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();

    let sqlite_pool = sqlx::SqlitePool::connect(&env.database_url).await?;

    let state = AppState {
        title_to_dense_id: Arc::new(load_title_to_dense_id_mmap()?),
        dense_id_to_title: Arc::new(load_dense_id_to_title_mmap()?),
        dense_id_to_orig: Arc::new(load_dense_id_to_orig_mmap()?),
        orig_to_dense_id: Arc::new(load_orig_to_dense_id_mmap()?),
        redirects_passed: Arc::new(load_redirects_passed_mmap()?),
        redirect_targets_dense: Arc::new(load_redirect_targets_dense_mmap()?),
        csr_graph: Arc::new(load_csr_graph_mmap()?),
        redis_pool: redis_pool,
        sqlite_pool: sqlite_pool,
        env: env,
    };

    let state = Arc::new(state); // one shared instance
    populate_leaderboard(
        &state.sqlite_pool,
        &state.redis_pool,
        "longest",
        "path_length",
        state.env.leaderboard_limit,
    )
    .await?;
    populate_leaderboard(
        &state.sqlite_pool,
        &state.redis_pool,
        "most",
        "num_paths",
        state.env.leaderboard_limit,
    )
    .await?;

    let cors = CorsLayer::new()
        // .allow_origin(Any) // allow all origins (for dev)
        .allow_origin(
            "http://localhost:5173"
                .parse::<axum::http::HeaderValue>()
                .unwrap(),
        )
        .allow_methods(Method::GET)
        .allow_headers(vec![CONTENT_TYPE, ACCEPT])
        .allow_credentials(true);

    // rate limiting
    // let subscriber = tracing_subscriber::FmtSubscriber::new();
    // tracing::subscriber::set_global_default(subscriber).unwrap();

    let governor_conf = GovernorConfigBuilder::default()
        .per_second(2)
        .burst_size(5)
        .finish()
        .unwrap();

    let governor_limiter = governor_conf.limiter().clone();
    let interval = Duration::from_secs(60);

    std::thread::spawn(move || loop {
        std::thread::sleep(interval);
        tracing::info!("rate limiting storage size: {}", governor_limiter.len());
        governor_limiter.retain_recent();
    });

    let app = Router::new()
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
        // .layer(GovernorLayer::new(governor_conf))
        .layer(cors);

    let addr = format!("0.0.0.0:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    // axum::serve(listener, app.into_make_service()).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();

    // search::benchmark_random_bfs(
    //     &csr_graph_mmap,
    //     &redirect_targets_dense,
    //     1000,
    //     255,
    //     &redirects_passed_mmap,
    // );

    // loop {
    //     thread::sleep(Duration::from_secs(60));
    // }

    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);

    Ok(())
}
