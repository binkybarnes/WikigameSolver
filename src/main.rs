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

use crate::parsers::pagelinks_parser::CsrGraph;
use crate::parsers::pagelinks_parser::CsrGraphMmap;
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

pub fn build_and_save_page_maps_dense() -> anyhow::Result<()> {
    let (orig_to_dense_id, dense_id_to_orig, title_to_dense_id, dense_id_to_title): (
        FxHashMap<u32, u32>,    // orig_to_dense_id
        Vec<u32>,               // dense_id_to_orig
        FxHashMap<String, u32>, // title_to_dense_id
        Vec<String>,            // dense_id_to_title
    ) = page_parser::build_title_maps_dense("../sql_files/enwiki-latest-page.sql.gz")?;

    util::save_to_file(&orig_to_dense_id, "data/orig_to_dense_id.bin")?;
    util::save_to_file(&dense_id_to_orig, "data/dense_id_to_orig.bin")?;
    util::save_to_file(&title_to_dense_id, "data/title_to_dense_id.bin")?;
    util::save_to_file(&dense_id_to_title, "data/dense_id_to_title.bin")?;

    Ok(())
}

pub fn build_and_save_redirect_targets_dense() -> anyhow::Result<()> {
    let title_to_dense_id: FxHashMap<String, u32> =
        util::load_from_file("data/title_to_dense_id.bin")?;
    let orig_to_dense_id: FxHashMap<u32, u32> = util::load_from_file("data/orig_to_dense_id.bin")?;

    let redirect_targets_dense = redirect_parser::build_redirect_targets_dense(
        "../sql_files/enwiki-latest-redirect.sql.gz",
        &title_to_dense_id,
        &orig_to_dense_id,
    )?;

    util::save_to_file(&redirect_targets_dense, "data/redirect_targets_dense.bin")?;

    Ok(())
}
pub fn build_and_save_linktargets_dense() -> anyhow::Result<()> {
    let title_to_id: FxHashMap<String, u32> = util::load_from_file("data/title_to_dense_id.bin")?;

    let linktargets_dense = linktarget_parser::build_linktargets_dense(
        "../sql_files/enwiki-latest-linktarget.sql.gz",
        &title_to_id,
    )?;

    util::save_to_file(&linktargets_dense, "data/linktargets_dense.bin")?;

    Ok(())
}

pub fn build_and_save_pagelinks_adj_list() -> anyhow::Result<()> {
    let linktargets_dense: FxHashMap<u32, u32> =
        util::load_from_file("data/linktargets_dense.bin")?;
    let redirect_targets_dense: FxHashMap<u32, u32> =
        util::load_from_file("data/redirect_targets_dense.bin")?;
    let orig_to_dense_id: FxHashMap<u32, u32> = util::load_from_file("data/orig_to_dense_id.bin")?;

    let (pagelinks_adjacency_list, incoming_pagelinks_adjacency_list, redirects_passed_dense): (
        FxHashMap<u32, Vec<u32>>,
        FxHashMap<u32, Vec<u32>>,
        FxHashMap<(u32, u32), u32>,
    ) = pagelinks_parser::build_pagelinks_dense(
        "../sql_files/enwiki-latest-pagelinks.sql.gz",
        &linktargets_dense,
        &redirect_targets_dense,
        &orig_to_dense_id,
    )?;

    util::save_to_file(
        &pagelinks_adjacency_list,
        "data/pagelinks_adjacency_list.bin",
    )?;
    util::save_to_file(
        &incoming_pagelinks_adjacency_list,
        "data/incoming_pagelinks_adjacency_list.bin",
    )?;
    util::save_to_file(&redirects_passed_dense, "data/redirects_passed_dense.bin")?;

    Ok(())
}

pub fn build_and_save_pagelinks_csr() -> anyhow::Result<()> {
    let pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
        util::load_from_file("data/pagelinks_adjacency_list.bin")?;
    let incoming_pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
        util::load_from_file("data/incoming_pagelinks_adjacency_list.bin")?;
    let orig_to_dense_id: FxHashMap<u32, u32> = util::load_from_file("data/orig_to_dense_id.bin")?;

    println!("building csr");
    let pagelinks_csr: pagelinks_parser::CsrGraph = pagelinks_parser::build_csr_with_adjacency_list(
        &orig_to_dense_id,
        &pagelinks_adjacency_list,
        &incoming_pagelinks_adjacency_list,
    );

    drop(pagelinks_adjacency_list);
    drop(incoming_pagelinks_adjacency_list);
    drop(orig_to_dense_id);

    // in memory version
    util::save_to_file(&pagelinks_csr, "data/pagelinks_csr.bin")?;

    // memory mappable version
    util::write_u32_vec_to_file(&pagelinks_csr.edges, "data/csr/edges.bin")?;
    util::write_u32_vec_to_file(&pagelinks_csr.reverse_edges, "data/csr/reverse_edges.bin")?;

    util::save_to_file(&pagelinks_csr.offsets, "data/csr/offsets.bin")?;
    util::save_to_file(
        &pagelinks_csr.reverse_offsets,
        "data/csr/reverse_offsets.bin",
    )?;

    Ok(())
}

pub fn load_csr_graph_mmap() -> anyhow::Result<CsrGraphMmap> {
    // Memory-map the big edge arrays
    let edges_mmap: Mmap = util::mmap_file("data/csr/edges.bin")?;
    let reverse_edges_mmap: Mmap = util::mmap_file("data/csr/reverse_edges.bin")?;

    // Load smaller arrays / maps into memory
    let offsets: Vec<u32> = util::load_from_file("data/csr/offsets.bin")?;
    let reverse_offsets: Vec<u32> = util::load_from_file("data/csr/reverse_offsets.bin")?;

    Ok(CsrGraphMmap {
        offsets,
        reverse_offsets,
        edges_mmap,
        reverse_edges_mmap,
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

pub fn build_and_save_redirects_passed_mmap() -> anyhow::Result<()> {
    // Load original data
    let redirects_passed_dense: FxHashMap<(u32, u32), u32> =
        util::load_from_file("data/redirects_passed_dense.bin")?;
    let dense_id_to_orig: Vec<u32> = util::load_from_file("data/dense_id_to_orig.bin")?;
    let num_pages = dense_id_to_orig.len();

    // Step 1: group by page_from
    let mut grouped: FxHashMap<u32, Vec<(u32, u32)>> = FxHashMap::default();
    for (&(page_from, target), &redir) in redirects_passed_dense.iter() {
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
    util::write_u32_vec_to_file(
        &redirect_targets,
        "data/redirects_passed/redirect_targets.bin",
    )?;
    util::write_u32_vec_to_file(&redirects, "data/redirects_passed/redirects.bin")?;

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

pub fn build_and_save_title_to_dense_id_mmap() -> anyhow::Result<()> {
    let title_to_dense_id: FxHashMap<String, u32> =
        util::load_from_file("data/title_to_dense_id.bin")?;

    // Step 1: collect (title, dense_id) pairs and sort by title
    let mut entries: Vec<(&String, &u32)> = title_to_dense_id.iter().collect();
    entries.sort_by(|(a_title, _), (b_title, _)| a_title.cmp(b_title));

    // Step 2: build UTF-8 blob + offsets + dense_ids
    let mut titles_blob: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(entries.len());
    let mut dense_ids: Vec<u32> = Vec::with_capacity(entries.len());

    for (title, dense_id) in entries {
        offsets.push(titles_blob.len() as u32);
        titles_blob.extend_from_slice(title.as_bytes());
        dense_ids.push(*dense_id);
    }

    // Step 3: save
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

pub fn build_and_save_dense_id_to_title_mmap() -> anyhow::Result<()> {
    let dense_id_to_title: Vec<String> = util::load_from_file("data/dense_id_to_title.bin")?;

    // Prepare the flat titles buffer and offsets
    let mut titles: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(dense_id_to_title.len() + 1);
    offsets.push(0);

    for title in &dense_id_to_title {
        titles.extend_from_slice(title.as_bytes());
        offsets.push(titles.len() as u32);
    }

    // Save to disk
    util::write_u8_vec_to_file(&titles, "data/dense_id_to_title/titles.bin")?;
    util::save_to_file(&offsets, "data/dense_id_to_title/offsets.bin")?;

    Ok(())
}

pub fn load_dense_id_to_title_mmap() -> anyhow::Result<DenseIdToTitleMmap> {
    let titles: Mmap = util::mmap_file("data/dense_id_to_title/titles.bin")?;
    let offsets: Vec<u32> = util::load_from_file("data/dense_id_to_title/offsets.bin")?;

    Ok(DenseIdToTitleMmap { titles, offsets })
}

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

    // load normal structures
    let redirect_targets_dense: FxHashMap<u32, u32> =
        util::load_from_file("data/redirect_targets_dense.bin")?;
    // let csr_graph: CsrGraph = util::load_from_file("data/pagelinks_csr.bin")?;

    // load mmap structures
    let title_to_dense_id_mmap: TitleToDenseIdMmap = load_title_to_dense_id_mmap()?;
    let dense_id_to_title_mmap: DenseIdToTitleMmap = load_dense_id_to_title_mmap()?;
    let redirects_passed_mmap: RedirectsPassedMmap = load_redirects_passed_mmap()?;
    let csr_graph_mmap: CsrGraphMmap = load_csr_graph_mmap()?;

    search::bfs_interactive_session(
        &title_to_dense_id_mmap,
        &dense_id_to_title_mmap,
        &csr_graph_mmap,
        &redirect_targets_dense,
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

    // let orig_to_dense_id: FxHashMap<u32, u32> = util::load_from_file("data/orig_to_dense_id.bin")?;
    // println!("{}", orig_to_dense_id.get(&53251).unwrap());

    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);

    Ok(())
}
