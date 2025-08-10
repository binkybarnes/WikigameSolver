mod parsers;
mod search;
mod util;
use parsers::linktarget_parser;
use parsers::page_parser;
use parsers::pagelinks_parser;
use parsers::redirect_parser;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    process::id,
    thread,
    time::{Duration, Instant},
};

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

pub fn build_and_save_page_maps() -> anyhow::Result<()> {
    let (title_to_id, id_to_title): (FxHashMap<String, u32>, FxHashMap<u32, String>) =
        page_parser::build_title_maps("../sql_files/enwiki-latest-page.sql.gz")?;

    util::save_to_file(&title_to_id, "data/title_to_id.bin")?;
    util::save_to_file(&id_to_title, "data/id_to_title.bin")?;

    Ok(())
}

pub fn build_and_save_redirect_targets() -> anyhow::Result<()> {
    let title_to_id: FxHashMap<String, u32> = util::load_from_file("data/title_to_id.bin")?;

    let redirect_targets = redirect_parser::build_redirect_targets(
        "../sql_files/enwiki-latest-redirect.sql.gz",
        &title_to_id,
    )?;

    util::save_to_file(&redirect_targets, "data/redirect_targets.bin")?;

    Ok(())
}
pub fn build_and_save_linktargets() -> anyhow::Result<()> {
    let title_to_id: FxHashMap<String, u32> = util::load_from_file("data/title_to_id.bin")?;

    let linktargets = linktarget_parser::build_linktargets(
        "../sql_files/enwiki-latest-linktarget.sql.gz",
        &title_to_id,
    )?;

    util::save_to_file(&linktargets, "data/linktargets.bin")?;

    Ok(())
}

pub fn build_and_save_page_links() -> anyhow::Result<()> {
    let linktargets: FxHashMap<u32, u32> = util::load_from_file("data/linktargets.bin")?;
    let pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> = pagelinks_parser::build_pagelinks(
        "../sql_files/enwiki-latest-pagelinks.sql.gz",
        &linktargets,
    )?;

    println!("building csr");
    let redirect_targets: FxHashMap<u32, u32> = util::load_from_file("data/redirect_targets.bin")?;
    let id_to_title: FxHashMap<u32, String> = util::load_from_file("data/id_to_title.bin")?;
    let pagelinks_csr: pagelinks_parser::CsrGraph = pagelinks_parser::build_csr_with_adjacency_list(
        &id_to_title,
        &pagelinks_adjacency_list,
        &redirect_targets,
    );

    util::save_to_file(
        &pagelinks_adjacency_list,
        "data/pagelinks_adjacency_list.bin",
    )?;
    util::save_to_file(&pagelinks_csr, "data/pagelinks_csr.bin")?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let now = Instant::now();

    // build_and_save_page_maps()?;
    // build_and_save_redirect_targets()?;
    // build_and_save_linktargets()?;
    // build_and_save_page_links()?;

    let id_to_title: FxHashMap<u32, String> = util::load_from_file("data/id_to_title.bin")?;
    let title_to_id: FxHashMap<String, u32> = util::load_from_file("data/title_to_id.bin")?;
    let redirect_targets: FxHashMap<u32, u32> = util::load_from_file("data/redirect_targets.bin")?;
    let linktargets: FxHashMap<u32, u32> = util::load_from_file("data/linktargets.bin")?;
    let pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
        util::load_from_file("data/pagelinks_adjacency_list.bin")?;
    let pagelinks_csr: pagelinks_parser::CsrGraph = util::load_from_file("data/pagelinks_csr.bin")?;

    println!("loaded");
    println!(
        "id_to_title len: {} redirect_targets len: {}",
        id_to_title.len(),
        redirect_targets.len()
    );
    // util::run_interactive_session(
    //     &title_to_id,
    //     &id_to_title,
    //     &redirect_targets,
    //     &linktargets,
    //     &pagelinks_adjacency_list,
    //     &pagelinks_csr,
    // )?;

    search::bfs_interactive_session(
        &title_to_id,
        &id_to_title,
        &pagelinks_adjacency_list,
        &pagelinks_csr,
        &redirect_targets,
    );

    // loop {
    //     thread::sleep(Duration::from_secs(60));
    // }

    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);

    Ok(())
}
