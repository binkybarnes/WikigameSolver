mod builders;
mod graph;
mod mmap_structs;
mod parsers;
mod search;
mod util;

use core::num;
use std::hash::Hash;
use std::net::SocketAddr;
use std::time::Duration;
use std::time::Instant;

use crate::builders::*;
use crate::graph::*;
use crate::mmap_structs::*;

use axum::body::Body;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use clap::Parser;
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

use rustc_hash::FxBuildHasher;
use rustc_hash::FxHashMap;
use rustc_hash::FxHasher;
use serde_json::to_vec;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    title_to_dense_id: Arc<TitleToDenseIdMmap>,
    dense_id_to_title: Arc<DenseIdToTitleMmap>,
    orig_to_dense_id: Arc<OrigToDenseIdMmap>,
    dense_id_to_orig: Arc<DenseIdToOrigMmap>,
    redirects_passed: Arc<RedirectsPassedMmap>,
    redirect_targets_dense: Arc<RedirectTargetsDenseMmap>,
    csr_graph: Arc<CsrGraphMmap>,
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
    num_paths: usize,
    elapsed_s: f64,
    paths: Vec<Vec<PathNode>>,
}

use axum::{
    http::{
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
        Request,
    },
    response::IntoResponse,
};

fn json_error(body: serde_json::Value, status: u16) -> Response {
    let body_bytes = serde_json::to_vec(&body).unwrap();
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(body_bytes))
        .unwrap()
}

use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

#[axum::debug_handler]
pub async fn search_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<PathRequest>,
) -> impl IntoResponse {
    let start_req = Instant::now();

    // --- Resolve start ---
    let start_id = match (&req.start, &req.start_id) {
        (Some(title), None) => match state.title_to_dense_id.get(title) {
            Some(id) => id,
            None => {
                return json_error(
                    json!({"error": format!("Start title '{}' not found. Check capitalization", title)}),
                    404,
                )
            }
        },
        (None, Some(orig_id)) => match state.orig_to_dense_id.get(*orig_id) {
            Some(id) => id,
            None => {
                return json_error(
                    json!({"error": format!("Start original ID '{}' not found", orig_id)}),
                    404,
                )
            }
        },
        _ => {
            return json_error(
                json!({"error": "Exactly one of start or start_id must be provided"}),
                400,
            )
        }
    };

    // --- Resolve end ---
    let goal_id = match (&req.end, &req.end_id) {
        (Some(title), None) => match state.title_to_dense_id.get(title) {
            Some(id) => id,
            None => {
                return json_error(
                    json!({"error": format!("End title '{}' not found. Check capitalization", title)}),
                    404,
                )
            }
        },
        (None, Some(orig_id)) => match state.orig_to_dense_id.get(*orig_id) {
            Some(id) => id,
            None => {
                return json_error(
                    json!({"error": format!("End original ID '{}' not found", orig_id)}),
                    404,
                )
            }
        },
        _ => {
            return json_error(
                json!({"error": "Exactly one of end or end_id must be provided"}),
                400,
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
    let result = search::bi_bfs_csr(
        &*state.csr_graph,
        start_id,
        goal_id,
        50,
        &state.redirects_passed,
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

    let num_paths = paths.len();

    let response = PathResponse {
        num_paths,
        elapsed_s,
        paths,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let now = Instant::now();

    let args = Args::parse();

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

    let state = AppState {
        title_to_dense_id: Arc::new(load_title_to_dense_id_mmap()?),
        dense_id_to_title: Arc::new(load_dense_id_to_title_mmap()?),
        dense_id_to_orig: Arc::new(load_dense_id_to_orig_mmap()?),
        orig_to_dense_id: Arc::new(load_orig_to_dense_id_mmap()?),
        redirects_passed: Arc::new(load_redirects_passed_mmap()?),
        redirect_targets_dense: Arc::new(load_redirect_targets_dense_mmap()?),
        csr_graph: Arc::new(load_csr_graph_mmap()?),
    };

    let state = Arc::new(state); // one shared instance

    let cors = CorsLayer::new()
        .allow_origin(Any) // allow all origins (for dev)
        .allow_methods(Any)
        .allow_headers(Any);

    // rate limiting
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber).unwrap();

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
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(GovernorLayer::new(governor_conf))
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
