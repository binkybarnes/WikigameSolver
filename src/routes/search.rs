use std::sync::Arc;
use std::time::Instant;

// src/routes/search.rs
use crate::leaderboard::try_add_to_leaderboard;
use crate::models::{PathNode, PathRequest, PathResponse};
use crate::search;
use crate::state::AppState;
use crate::util::json_response;
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use axum::{extract::State, response::IntoResponse, Extension, Json};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

#[axum::debug_handler]
pub async fn search_handler(
    State(state): State<Arc<AppState>>,
    // headers: HeaderMap,
    // Extension(user_id): Extension<String>,
    maybe_user: Option<Extension<String>>,
    Json(req): Json<PathRequest>,
) -> impl IntoResponse {
    let start_req = Instant::now();

    let user_id: String = maybe_user
        .map(|Extension(s)| s)
        .unwrap_or_else(|| "NO_USER".to_string());

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
        &user_id,
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
        &user_id,
        state.env.leaderboard_limit,
    )
    .await;

    tracing::debug!("db stuff took {:?}", sql_time.elapsed());
    let response = PathResponse {
        elapsed_s,
        paths,
        leaderboard_longest_rank,
        leaderboard_most_rank,
    };

    // Optional: log size
    let body = serde_json::to_vec(&response).unwrap();

    tracing::debug!("Response size: {} bytes", body.len());

    tracing::debug!("Response time: {:.2?}\n", start_req.elapsed());

    // Build response with headers
    Response::builder()
        .header("Content-Type", "application/json")
        // .header("ETag", etag)
        .header("Cache-Control", "public, max-age=31536000, immutable") // ~1 year
        .body(Body::from(body))
        .unwrap()

    // Json(json!(response))
}
