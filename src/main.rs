mod page_parser;
mod pagelinks_parser;
mod redirect_parser;
mod util;
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

pub fn build_and_save_page_links() -> anyhow::Result<()> {
    let id_to_title: FxHashMap<u32, String> = util::load_from_file("data/id_to_title.bin")?;
    let redirect_targets: FxHashMap<u32, u32> = util::load_from_file("data/redirect_targets.bin")?;

    let page_links = pagelinks_parser::build_pagelinks(
        "../sql_files/enwiki-latest-pagelinks.sql.gz",
        &id_to_title,
        &redirect_targets,
    )?;

    util::save_to_file(&page_links, "data/page_links.bin")?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let now = Instant::now();

    // build_and_save_page_maps()?;
    // build_and_save_redirect_targets()?;
    build_and_save_page_links()?;

    // let page_links: FxHashMap<u32, Vec<u32>> = util::load_from_file("data/page_links.bin")?;
    // println!("loaded");
    // util::run_interactive_session(&title_to_id, &id_to_title, &redirect_targets)?;

    // loop {
    //     thread::sleep(Duration::from_secs(60));
    // }

    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);

    Ok(())
}
