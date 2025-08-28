use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use redis::AsyncCommands;
use rustc_hash::FxHasher;
use serde::{Deserialize, Serialize};
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::state::AppState; // assumes redis + sqlite + dense_to_title

#[derive(Serialize)]
pub struct LeaderboardEntry {
    start_id: u32,
    end_id: u32,
    score: u32,
    username: String,
    rank: usize,
}

#[derive(Deserialize)]
pub struct LeaderboardQuery {
    offset: Option<usize>, // default 0
    limit: Option<usize>,  // default 50
}

pub async fn get_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(leaderboard_type): Path<String>,
    Query(params): Query<LeaderboardQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let leaderboard_key = format!("leaderboard:{}", leaderboard_type);
    let username_hash_key = "leaderboard:username";

    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50);
    let stop = offset + limit - 1;

    let mut conn = state.redis_pool.get().await.unwrap();

    // 1️⃣ Get top 500 entries from the sorted set
    let entries: Vec<(String, u32)> = conn
        .zrevrange_withscores(&leaderboard_key, offset as isize, stop as isize)
        .await
        .unwrap();

    // 2️⃣ Build Redis hash keys to fetch usernames
    let hash_keys: Vec<String> = entries
        .iter()
        .map(|(path, _)| format!("{}|{}", path, leaderboard_type))
        .collect();

    // 3️⃣ Fetch all usernames in a single HMGET
    let usernames: Vec<Option<String>> = conn.hmget(&username_hash_key, &hash_keys).await.unwrap();

    // 4️⃣ Build leaderboard entries
    let result: Vec<LeaderboardEntry> = entries
        .into_iter()
        .zip(usernames.into_iter())
        .enumerate()
        .filter_map(|(i, ((path, score), username_opt))| {
            let username = username_opt?;
            let mut parts = path.split('|');
            let start_id: u32 = parts.next()?.parse().ok()?;
            let end_id: u32 = parts.next()?.parse().ok()?;
            Some(LeaderboardEntry {
                start_id,
                end_id,
                score,
                username,
                rank: offset + i,
            })
        })
        .collect();

    // --- Caching Logic ---
    let body_bytes = serde_json::to_vec(&result).unwrap();

    // 1. Generate ETag from the response body
    let mut hasher = FxHasher::default();
    body_bytes.hash(&mut hasher);
    let etag = format!("{:x}", hasher.finish());

    // 2. Check if the browser sent an If-None-Match header
    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
        if if_none_match.to_str().unwrap_or_default() == etag {
            // 3. If ETags match, return 304 Not Modified
            return (StatusCode::NOT_MODIFIED, "").into_response();
        }
    }

    // 4. Otherwise, send the full response with caching headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ETAG, etag)
        .header(header::CACHE_CONTROL, "public, max-age=600") // Cache for 10 minutes
        .body(Body::from(body_bytes))
        .unwrap()
}
