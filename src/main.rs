mod parsers;
mod search;
mod util;
use memmap2::Mmap;
use parsers::linktarget_parser;
use parsers::page_parser;
use parsers::pagelinks_parser;
use parsers::redirect_parser;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::cmp::Ordering;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    process::id,
    thread,
    time::{Duration, Instant},
};

use crate::parsers::pagelinks_parser::CsrGraphTrait;

use crate::util::save_to_file;

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
    // let linktargets: FxHashMap<u32, u32> = util::load_from_file("data/linktargets.bin")?;
    // let redirect_targets: FxHashMap<u32, u32> = util::load_from_file("data/redirect_targets.bin")?;
    // let (pagelinks_adjacency_list, incoming_pagelinks_adjacency_list, redirects_passed): (
    //     FxHashMap<u32, Vec<u32>>,
    //     FxHashMap<u32, Vec<u32>>,
    //     FxHashMap<(u32, u32), u32>,
    // ) = pagelinks_parser::build_pagelinks(
    //     "../sql_files/enwiki-latest-pagelinks.sql.gz",
    //     &linktargets,
    //     &redirect_targets,
    // )?;

    let pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
        util::load_from_file("data/pagelinks_adjacency_list.bin")?;
    let incoming_pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
        util::load_from_file("data/incoming_pagelinks_adjacency_list.bin")?;
    let redirects_passed: FxHashMap<(u32, u32), u32> =
        util::load_from_file("data/redirects_passed.bin")?;

    println!("building csr");
    let id_to_title: FxHashMap<u32, String> = util::load_from_file("data/id_to_title.bin")?;
    let pagelinks_csr: pagelinks_parser::CsrGraph = pagelinks_parser::build_csr_with_adjacency_list(
        &id_to_title,
        &pagelinks_adjacency_list,
        &incoming_pagelinks_adjacency_list,
        &redirects_passed,
    );

    util::save_to_file(&redirects_passed, "data/redirects_passed.bin")?;
    util::save_to_file(
        &pagelinks_adjacency_list,
        "data/pagelinks_adjacency_list.bin",
    )?;
    util::save_to_file(
        &incoming_pagelinks_adjacency_list,
        "data/incoming_pagelinks_adjacency_list.bin",
    )?;

    // util::save_to_file(&pagelinks_csr, "data/pagelinks_csr.bin")?;
    util::write_u32_vec_to_file(&pagelinks_csr.edges, "data/csr/edges.bin")?;
    util::write_u32_vec_to_file(&pagelinks_csr.reverse_edges, "data/csr/reverse_edges.bin")?;

    util::save_to_file(&pagelinks_csr.offsets, "data/csr/offsets.bin")?;
    util::save_to_file(
        &pagelinks_csr.reverse_offsets,
        "data/csr/reverse_offsets.bin",
    )?;
    util::save_to_file(&pagelinks_csr.orig_to_dense, "data/csr/orig_to_dense.bin")?;
    util::save_to_file(&pagelinks_csr.dense_to_orig, "data/csr/dense_to_orig.bin")?;
    util::save_to_file(
        &pagelinks_csr.redirects_passed,
        "data/csr/redirects_passed.bin",
    )?;

    Ok(())
}

pub fn load_csr_graph_mmap() -> anyhow::Result<pagelinks_parser::CsrGraphMmap> {
    // Memory-map the big edge arrays
    let edges_mmap: Mmap = util::mmap_file("data/csr/edges.bin")?;
    let reverse_edges_mmap: Mmap = util::mmap_file("data/csr/reverse_edges.bin")?;
    let redirects_passed: RedirectsPassedMmap = load_redirects_passed_mmap()?;

    // Load smaller arrays / maps into memory
    let offsets: Vec<u32> = util::load_from_file("data/csr/offsets.bin")?;
    let reverse_offsets: Vec<u32> = util::load_from_file("data/csr/reverse_offsets.bin")?;
    let orig_to_dense: FxHashMap<u32, u32> = util::load_from_file("data/csr/orig_to_dense.bin")?;
    let dense_to_orig: Vec<u32> = util::load_from_file("data/csr/dense_to_orig.bin")?;
    // let redirects_passed: FxHashMap<(u32, u32), u32> =
    //     util::load_from_file("data/csr/redirects_passed.bin")?;

    Ok(pagelinks_parser::CsrGraphMmap {
        offsets,
        reverse_offsets,
        edges_mmap,
        reverse_edges_mmap,
        orig_to_dense,
        dense_to_orig,
        redirects_passed,
    })
}

// going to replace pub redirects_passed: FxHashMap<(u32, u32), u32>, inside of csr_graph
pub struct RedirectsPassedMmap {
    pub offsets: Vec<u32>,
    pub redirect_targets_mmap: Mmap,
    pub redirects_mmap: Mmap,
    // dense ids
}

pub trait RedirectsPassedTrait {
    fn get_redirect(&self, from: u32, to: u32) -> Option<u32>;
}
impl RedirectsPassedTrait for FxHashMap<(u32, u32), u32> {
    fn get_redirect(&self, from: u32, to: u32) -> Option<u32> {
        self.get(&(from, to)).copied()
    }
}

impl RedirectsPassedTrait for RedirectsPassedMmap {
    fn get_redirect(&self, from: u32, to: u32) -> Option<u32> {
        self.get(from, to)
    }
}

impl RedirectsPassedMmap {
    /// Return the redirect for a given (page_from, target_id) if it exists
    pub fn get(&self, page_from: u32, target_id: u32) -> Option<u32> {
        let offsets = &self.offsets;
        let start = offsets[page_from as usize] as usize;
        let end = offsets[page_from as usize + 1] as usize;

        let targets_bytes: &[u8] = &self.redirect_targets_mmap;
        let redirects_bytes: &[u8] = &self.redirects_mmap;

        // Interpret bytes as u32 array (little endian assumed)
        let targets: &[u32] = bytemuck::cast_slice(&targets_bytes[start * 4..end * 4]);
        let redirects: &[u32] = bytemuck::cast_slice(&redirects_bytes[start * 4..end * 4]);

        // Binary search within this page's subsection
        let mut lo = 0usize;
        let mut hi = targets.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            match targets[mid].cmp(&target_id) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(redirects[mid]),
            }
        }
        None
    }
}

pub fn build_and_save_redirects_passed() -> anyhow::Result<()> {
    // Load original data
    let redirects_passed: FxHashMap<(u32, u32), u32> =
        util::load_from_file("data/csr/redirects_passed.bin")?;
    let orig_to_dense: FxHashMap<u32, u32> = util::load_from_file("data/csr/orig_to_dense.bin")?;

    let num_pages = orig_to_dense.len();

    // Step 1: group by page_from
    let mut grouped: FxHashMap<u32, Vec<(u32, u32)>> = FxHashMap::default();
    for (&(page_from, target), &redir) in redirects_passed.iter() {
        grouped.entry(page_from).or_default().push((target, redir));
    }

    // Step 2: sort each page_from's redirects by target
    for vec in grouped.values_mut() {
        vec.sort_by_key(|&(target, _)| target);
    }

    // Step 3: build offsets, redirect_targets, and redirects
    let mut offsets = Vec::with_capacity(num_pages + 1);
    let mut redirect_targets: Vec<u32> = Vec::new();
    let mut redirects: Vec<u32> = Vec::new();
    offsets.push(0);

    for page_id in 0..num_pages {
        if let Some(pairs) = grouped.get(&(page_id as u32)) {
            for &(target, redir) in pairs {
                redirect_targets.push(target);
                redirects.push(redir);
            }
        }
        offsets.push(redirect_targets.len() as u32);
    }

    // Step 4: save to disk
    util::save_to_file(&offsets, "data/redirects_passed/offsets.bin")?;
    util::save_to_file(
        &redirect_targets,
        "data/redirects_passed/redirect_targets.bin",
    )?;
    util::save_to_file(&redirects, "data/redirects_passed/redirects.bin")?;

    Ok(())
}

pub fn load_redirects_passed_mmap() -> anyhow::Result<RedirectsPassedMmap> {
    let offsets: Vec<u32> = util::load_from_file("data/redirects_passed/offsets.bin")?;
    let redirect_targets_mmap = util::mmap_file("data/redirects_passed/redirect_targets.bin")?;
    let redirects_mmap = util::mmap_file("data/redirects_passed/redirects.bin")?;

    Ok(RedirectsPassedMmap {
        offsets,
        redirect_targets_mmap,
        redirects_mmap,
    })
}

pub struct TitleToDenseIdMmap {
    // sorted so can perform binary search
    pub titles: Mmap,
    pub offsets: Vec<u32>,
    pub dense_ids: Vec<u32>,
}

impl TitleToDenseIdMmap {
    // given title, return dense id via binary search
    // courtesy of mister gippity
    pub fn get(&self, title: &str) -> Option<u32> {
        let bytes = &self.titles[..];
        let needle = title.as_bytes();

        let n = self.dense_ids.len(); // number of titles
        let mut lo = 0usize;
        let mut hi = n; // search in [lo, hi)

        while lo < hi {
            let mid = (lo + hi) / 2;
            let start = self.offsets[mid] as usize;
            // Support either offsets.len() == n or n+1 (with sentinel)
            let end = if mid + 1 < self.offsets.len() {
                self.offsets[mid + 1] as usize
            } else {
                bytes.len()
            };

            let s_bytes = &bytes[start..end];
            match s_bytes.cmp(needle) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(self.dense_ids[mid]),
            }
        }
        None
    }
}

pub fn build_and_save_title_to_dense_id() -> anyhow::Result<()> {
    let title_to_orig: FxHashMap<String, u32> = util::load_from_file("data/title_to_id.bin")?;
    let orig_to_dense: FxHashMap<u32, u32> = util::load_from_file("data/csr/orig_to_dense.bin")?;

    // Step 1: collect (title, dense_id) pairs
    let mut entries: Vec<(String, u32)> = title_to_orig
        .into_iter()
        .filter_map(|(title, orig_id)| {
            orig_to_dense
                .get(&orig_id)
                .map(|&dense_id| (title, dense_id))
        })
        .collect();

    // Step 2: sort by title (lexicographically)
    entries.sort_by(|(title_a, _), (title_b, _)| title_a.cmp(title_b));

    // Step 3: build UTF-8 blob + offsets + dense_ids
    let mut titles_blob: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(entries.len());
    let mut dense_ids: Vec<u32> = Vec::with_capacity(entries.len());

    for (title, dense_id) in entries {
        offsets.push(titles_blob.len() as u32);
        titles_blob.extend_from_slice(title.as_bytes());
        dense_ids.push(dense_id);
    }

    // Step 4: save
    util::write_u8_vec_to_file(&titles_blob, "data/title_to_dense_id/titles.bin")?;
    util::save_to_file(&offsets, "data/title_to_dense_id/offsets.bin")?;
    util::save_to_file(&dense_ids, "data/title_to_dense_id/dense_ids.bin")?;

    Ok(())
}

pub fn load_title_to_dense_id_mmap() -> anyhow::Result<TitleToDenseIdMmap> {
    let titles = util::mmap_file("data/title_to_dense_id/titles.bin")?;
    let offsets: Vec<u32> = util::load_from_file("data/title_to_dense_id/offsets.bin")?;
    let dense_ids: Vec<u32> = util::load_from_file("data/title_to_dense_id/dense_ids.bin")?;

    Ok(TitleToDenseIdMmap {
        titles,
        offsets,
        dense_ids,
    })
}

pub struct DenseIdToTitleMmap {
    pub titles: Mmap,
    pub offsets: Vec<u32>,
}

impl DenseIdToTitleMmap {
    pub fn get(&self, dense_id: u32) -> &str {
        let start = self.offsets[dense_id as usize] as usize;
        let end = self.offsets[dense_id as usize + 1] as usize;
        std::str::from_utf8(&self.titles[start..end]).unwrap()
    }
}

pub fn build_and_save_dense_id_to_title() -> anyhow::Result<()> {
    let orig_id_to_title: FxHashMap<u32, String> = util::load_from_file("data/id_to_title.bin")?;
    let dense_to_orig: Vec<u32> = util::load_from_file("data/csr/dense_to_orig.bin")?;

    let mut titles: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(dense_to_orig.len() + 1);
    offsets.push(0);

    for orig_id in dense_to_orig {
        let title = &orig_id_to_title[&orig_id];
        titles.extend_from_slice(title.as_bytes());
        offsets.push(titles.len() as u32);
    }

    util::write_u8_vec_to_file(&titles, "data/dense_id_to_title/titles.bin")?;
    util::save_to_file(&offsets, "data/dense_id_to_title/offsets.bin")?;

    Ok(())
}

pub fn load_dense_id_to_title_mmap() -> anyhow::Result<DenseIdToTitleMmap> {
    let titles: Mmap = util::mmap_file("data/dense_id_to_title/titles.bin")?;
    let offsets: Vec<u32> = util::load_from_file("data/dense_id_to_title/offsets.bin")?;

    Ok(DenseIdToTitleMmap { titles, offsets })
}

pub fn build_and_save_dense_redirect_targets() -> anyhow::Result<()> {
    // Load original maps
    let redirect_targets: FxHashMap<u32, u32> = util::load_from_file("data/redirect_targets.bin")?;
    let orig_to_dense: FxHashMap<u32, u32> = util::load_from_file("data/csr/orig_to_dense.bin")?;

    // New dense-to-dense map
    let mut dense_redirect_targets: FxHashMap<u32, u32> =
        FxHashMap::with_capacity_and_hasher(redirect_targets.len(), Default::default());

    for (orig_redirect, orig_target) in redirect_targets {
        if let (Some(&dense_redirect), Some(&dense_target)) = (
            orig_to_dense.get(&orig_redirect),
            orig_to_dense.get(&orig_target),
        ) {
            dense_redirect_targets.insert(dense_redirect, dense_target);
        }
    }

    util::save_to_file(&dense_redirect_targets, "data/dense_redirect_targets.bin")?;

    Ok(())
}
pub fn load_dense_redirect_targets() -> anyhow::Result<FxHashMap<u32, u32>> {
    let dense_redirect_targets: FxHashMap<u32, u32> =
        util::load_from_file("data/dense_redirect_targets.bin")?;

    Ok(dense_redirect_targets)
}

fn main() -> anyhow::Result<()> {
    let now = Instant::now();

    // build_and_save_page_maps()?;
    // build_and_save_redirect_targets()?;
    // build_and_save_linktargets()?;
    // build_and_save_page_links()?;

    // build_and_save_dense_id_to_title()?;
    // build_and_save_title_to_dense_id()?;
    // build_and_save_dense_redirect_targets()?;
    // build_and_save_redirects_passed()?;

    // let orig_id_to_title: FxHashMap<u32, String> = util::load_from_file("data/id_to_title.bin")?;
    // let title_to_orig_id: FxHashMap<String, u32> = util::load_from_file("data/title_to_id.bin")?;
    // let redirect_targets: FxHashMap<u32, u32> = util::load_from_file("data/redirect_targets.bin")?;

    // let redirects_passed: FxHashMap<(u32, u32), u32> =
    //     util::load_from_file("data/redirects_passed.bin")?;
    // let linktargets: FxHashMap<u32, u32> = util::load_from_file("data/linktargets.bin")?;
    // let pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
    //     util::load_from_file("data/pagelinks_adjacency_list.bin")?;
    // let incoming_pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
    //     util::load_from_file("data/incoming_pagelinks_adjacency_list.bin")?;
    // let pagelinks_csr: pagelinks_parser::CsrGraph = util::load_from_file("data/pagelinks_csr.bin")?;

    let pagelinks_csr = load_csr_graph_mmap()?;
    let dense_id_to_title = load_dense_id_to_title_mmap()?;
    let title_to_dense_id = load_title_to_dense_id_mmap()?;
    let dense_redirect_targets = load_dense_redirect_targets()?;

    println!("loaded");
    // println!(
    //     "id_to_title len: {} redirect_targets len: {}",
    //     id_to_title.len(),
    //     redirect_targets.len()
    // );
    // util::run_interactive_session(
    //     &title_to_id,
    //     &id_to_title,
    //     &redirect_targets,
    //     &linktargets,
    //     &pagelinks_adjacency_list,
    //     &incoming_pagelinks_adjacency_list,
    //     &pagelinks_csr,
    // )?;

    search::bfs_interactive_session(
        &title_to_dense_id,
        &dense_id_to_title,
        &pagelinks_csr,
        // &pagelinks_adjacency_list,
        // &incoming_pagelinks_adjacency_list,
        &dense_redirect_targets,
        // &redirects_passed,
    );

    // search::benchmark_random_bfs(&pagelinks_csr, &dense_redirect_targets, 1000, 8);

    // loop {
    //     thread::sleep(Duration::from_secs(60));
    // }

    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);

    Ok(())
}
