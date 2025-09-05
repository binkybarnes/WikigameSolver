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

use std::cmp::Reverse;
use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use std::time::Instant;

use crate::builders::*;
use crate::config::EnvironmentVariables;
use crate::graph::CsrGraphTrait;
use crate::leaderboard::populate_leaderboard;
use crate::mmap_structs::*;
use crate::routes::create_router;
use crate::state::AppState;

use clap::Parser;
use deadpool_redis::Config as RedisConfig;

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

    #[arg(long)]
    benchmark: bool,

    /// Port for the API server
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

use axum::http::Method;
use reqwest::header::ACCEPT;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::ETAG;
use reqwest::header::IF_NONE_MATCH;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::GovernorLayer;
use tower_http::cors::CorsLayer;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

use tracing_appender::non_blocking;
use tracing_appender::rolling;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

fn init_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    // Make logs directory
    std::fs::create_dir_all("logs").ok();

    // File appender, daily rotation
    let file_appender = rolling::daily("logs", "error.log");
    let (file_writer, file_guard) = non_blocking(file_appender);

    // Use EnvFilter so RUST_LOG is respected
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Layer that writes only ERROR+ to file
    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_filter(EnvFilter::new("error"));

    // Layer that writes INFO+ to stdout
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_filter(env_filter);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stdout_layer)
        .init();

    file_guard
}

pub fn find_sources_sinks<G: CsrGraphTrait>(g: &G, limit: usize) -> (Vec<u32>, Vec<u32>) {
    // Assumes num_nodes() is correct (offsets.len() - 1 for CSR)
    let n = g.num_nodes() as u32;
    let mut sources = Vec::with_capacity(limit.min(1024));
    let mut sinks = Vec::with_capacity(limit.min(1024));

    for u in 0..n {
        // Using lengths avoids touching edges beyond computing slice bounds.
        let outdeg = g.get(u).len();
        let indeg = g.get_reverse(u).len();

        if indeg == 0 && sources.len() < limit {
            sources.push(u);
        }
        if outdeg == 0 && sinks.len() < limit {
            sinks.push(u);
        }
        if sources.len() >= limit && sinks.len() >= limit {
            break;
        }
    }
    (sources, sinks)
}

use std::fs::File;
use std::io::{BufWriter, Write};

fn save_titles_to_file(
    path: &str,
    dense_ids: &[u32],
    dense_to_title: &dense_id_to_title::DenseIdToTitleMmap,
) -> std::io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    for &id in dense_ids {
        let title = dense_to_title.get(id);
        writeln!(writer, "{}", title)?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let now = Instant::now();

    if args.benchmark {
        // let dense_id_to_title = Arc::new(load_dense_id_to_title_mmap()?);
        // print_top_paths(
        //     "results/top_paths_1756748771.bin",
        //     dense_id_to_title.clone(),
        //     200,
        // )?;

        // return Ok(());

        let num_threads = num_cpus::get(); // logical cores
        println!("running on {} threads", num_threads);
        let filename = run_bfs_worker(num_threads, 10000, 5 * num_threads * 10000)?;
        println!("BFS finished. Results saved to {}", filename);
        println!("Elapsed: {:.2?}", now.elapsed());

        return Ok(());
    }
    // ------------------------------------------------------------------------

    let _guard = init_tracing();
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

    // let csr_graph = load_csr_graph_mmap()?;
    // let dense_to_title = load_dense_id_to_title_mmap()?;

    // let (sources, sinks) = find_sources_sinks(&csr_graph, 1000);
    // save_titles_to_file("sources.txt", &sources, &dense_to_title)?;
    // save_titles_to_file("sinks.txt", &sinks, &dense_to_title)?;

    // return Ok(());
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
        env: env.clone(),
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
            env.frontend_origin
                .parse::<axum::http::HeaderValue>()
                .unwrap(),
        )
        .allow_methods(Method::GET)
        .allow_headers(vec![CONTENT_TYPE, ACCEPT, IF_NONE_MATCH])
        .allow_credentials(true)
        .expose_headers([ETAG]);

    // rate limiting
    // let subscriber = tracing_subscriber::FmtSubscriber::new();
    // tracing::subscriber::set_global_default(subscriber).unwrap();

    let governor_conf = GovernorConfigBuilder::default()
        .per_second(4)
        .burst_size(5)
        .finish()
        .unwrap();

    let governor_limiter = governor_conf.limiter().clone();
    let interval = Duration::from_secs(3 * 60);

    std::thread::spawn(move || loop {
        std::thread::sleep(interval);
        tracing::info!("rate limiting storage size: {}", governor_limiter.len());
        governor_limiter.retain_recent();
    });

    // 6. Create Router from our routes module
    let mut app = create_router(state, cors);
    if env.is_production {
        app = app.layer(GovernorLayer::new(governor_conf));
    }

    // let app = Router::new()
    //     .route("/search", post(search_handler))
    //     .route("/me", get(me_handler))
    //     .route("/auth/google", post(google_auth_login_handler))
    //     .route("/auth/logout", post(logout_handler))
    //     .route("/user/change-username", post(change_username_handler))
    //     .layer(middleware::from_fn_with_state(
    //         state.clone(),
    //         jwt_middleware,
    //     ))
    //     .with_state(state)
    //     .layer(CookieManagerLayer::new())
    //     .layer(CompressionLayer::new())
    //     // .layer(GovernorLayer::new(governor_conf))
    //     .layer(cors);

    // let addr = format!("0.0.0.0:{}", args.port);
    let addr = format!("0.0.0.0:{}", env.port);
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

use rand::Rng;
use std::{
    collections::{BinaryHeap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

// import your BFS and graph code
use crate::load_csr_graph_mmap;
use crate::search::*;

/// Runs multithreaded BFS workers for random article pairs.
/// Stops on Enter key and saves top paths to a timestamped binary file.

pub fn run_bfs_worker(
    num_threads: usize,
    top_k_per_thread: usize,
    top_k_global: usize,
) -> anyhow::Result<String> {
    // Load graph
    let csr_graph = Arc::new(load_csr_graph_mmap()?);

    // Stoppable flag
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Thread-local BFS counters
    let bfs_counts: Arc<Vec<AtomicUsize>> =
        Arc::new((0..num_threads).map(|_| AtomicUsize::new(0)).collect());

    let start_time = Instant::now();

    // Spawn worker threads
    let mut handles = vec![];
    for i in 0..num_threads {
        let graph = csr_graph.clone();
        let stop = stop_flag.clone();
        let top_k = top_k_per_thread;
        let bfs_count = bfs_counts.clone();
        let handle = thread::spawn(move || {
            let mut heap: BinaryHeap<Reverse<(u8, u32, u32)>> = BinaryHeap::new();
            let mut rng = rand::rng();
            while !stop.load(Ordering::Relaxed) {
                let start = rng.random_range(0..graph.num_nodes()) as u32;
                let goal = rng.random_range(0..graph.num_nodes()) as u32;

                if let Some(depth) = bi_bfs_csr_depth_only(&*graph, start, goal) {
                    let rev_item = Reverse((depth, start, goal));
                    if heap.len() < top_k {
                        heap.push(rev_item);
                    } else if let Some(&Reverse((min_depth, _, _))) = heap.peek() {
                        if depth > min_depth {
                            heap.pop();
                            heap.push(rev_item);
                        }
                    }
                    bfs_count[i].fetch_add(1, Ordering::Relaxed);
                }
            }
            heap
        });
        handles.push(handle);
    }

    // Stop on Enter key
    println!("Press Enter to stop BFS workers...");
    let stop_clone = stop_flag.clone();
    thread::spawn(move || {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        stop_clone.store(true, Ordering::Relaxed);
    });

    // Wait for threads and collect their heaps
    let mut global_heap: BinaryHeap<Reverse<(u8, u32, u32)>> = BinaryHeap::new();
    for (i, handle) in handles.into_iter().enumerate() {
        let local_heap = handle.join().unwrap();
        for rev_item in local_heap {
            if global_heap.len() < top_k_global {
                global_heap.push(rev_item);
            } else if let Some(&Reverse((min_depth, _, _))) = global_heap.peek() {
                if rev_item.0 .0 > min_depth {
                    global_heap.pop();
                    global_heap.push(rev_item);
                }
            }
        }
    }

    // BFS statistics
    let total_bfs: usize = bfs_counts.iter().map(|c| c.load(Ordering::Relaxed)).sum();
    let elapsed_secs = start_time.elapsed().as_secs_f64();

    println!("Total BFS runs: {}", total_bfs);
    println!("Threads used: {}", num_threads);
    println!("Top K global paths: {}", top_k_global);

    println!("Total BFS runs: {}", total_bfs);
    println!("Elapsed time: {:.2} seconds", elapsed_secs);
    println!("BFS per second: {:.2}", total_bfs as f64 / elapsed_secs);

    // Count per depth
    let mut depth_counts: FxHashMap<u8, usize> = FxHashMap::default();
    for &Reverse((depth, _start, _goal)) in global_heap.iter() {
        *depth_counts.entry(depth).or_insert(0) += 1;
    }

    let mut depths: Vec<_> = depth_counts.keys().cloned().collect();
    depths.sort_unstable_by(|a, b| b.cmp(a));
    for depth in depths {
        println!("Depth {}: {}", depth, depth_counts[&depth]);
    }

    // Save to file
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let filename = format!("results/top_paths_{}.bin", timestamp);
    let vec_to_save: Vec<(u8, u32, u32)> = global_heap.into_iter().map(|Reverse(t)| t).collect();
    util::save_to_file(&vec_to_save, &filename)?;
    println!("Saved global heap to {}", filename);

    Ok(filename)
}

pub fn print_top_paths(
    path: &str,
    dense_id_to_title: Arc<DenseIdToTitleMmap>,
    top_k: usize,
) -> anyhow::Result<()> {
    // Load the saved vector from file
    let mut paths: Vec<(u8, u32, u32)> = util::load_from_file(path)?;

    // Sort by depth descending
    paths.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    println!("Top {} paths from file {}:", top_k, path);
    for (i, &(depth, start, goal)) in paths.iter().take(top_k).enumerate() {
        let start_title = dense_id_to_title.get(start);
        let goal_title = dense_id_to_title.get(goal);
        println!(
            "{}. Depth {}: {} -> {}",
            i + 1,
            depth,
            start_title,
            goal_title
        );
    }

    Ok(())
}
