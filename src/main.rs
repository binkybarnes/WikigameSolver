mod builders;
mod graph;
mod mmap_structs;
mod parsers;
mod search;
mod util;
use memmap2::Mmap;
use parsers::linktarget_parser;
use parsers::page_parser;
use parsers::pagelinks_parser;
use parsers::redirect_parser;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::any;
use std::cmp::Ordering;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    process::id,
    thread,
    time::{Duration, Instant},
};

use crate::graph::*;
use crate::mmap_structs::*;

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

fn main() -> anyhow::Result<()> {
    let now = Instant::now();

    // // build and save normal structures
    // build_and_save_page_maps_dense()?;
    // // ↓
    // build_and_save_linktargets_dense()?;
    // build_and_save_redirect_targets_dense()?;
    // // ↓
    // build_and_save_pagelinks_adj_list()?;
    // // ↓
    // build_and_save_pagelinks_csr()?;

    // // build and save mmap structures
    // build_and_save_title_to_dense_id_mmap()?;
    // build_and_save_dense_id_to_title_mmap()?;
    // build_and_save_pagelinks_csr()?;
    // build_and_save_redirects_passed_mmap()?;
    // build_and_save_redirect_targets_dense_mmap()?;

    // load normal structures
    // let csr_graph: CsrGraph = util::load_from_file("data/pagelinks_csr.bin")?;

    // // load mmap structures
    let title_to_dense_id_mmap: TitleToDenseIdMmap = load_title_to_dense_id_mmap()?;
    let dense_id_to_title_mmap: DenseIdToTitleMmap = load_dense_id_to_title_mmap()?;
    let redirects_passed_mmap: RedirectsPassedMmap = load_redirects_passed_mmap()?;
    let redirect_targets_dense_mmap: RedirectTargetsDenseMmap = load_redirect_targets_dense_mmap()?;
    let csr_graph_mmap: CsrGraphMmap = load_csr_graph_mmap()?;

    search::bfs_interactive_session(
        &title_to_dense_id_mmap,
        &dense_id_to_title_mmap,
        &csr_graph_mmap,
        &redirect_targets_dense_mmap,
        &redirects_passed_mmap,
    );

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
