use crate::util;
use bitcode::{Decode, Encode};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustc_hash::{FxBuildHasher, FxHashMap};

use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

#[derive(Encode, Decode)]
pub struct CsrGraph {
    pub offsets: Vec<u32>,
    pub edges: Vec<u32>,
    pub reverse_offsets: Vec<u32>,
    pub reverse_edges: Vec<u32>,
    // page_ids are sparse, so map each page_id to 1, 2, ...
    pub orig_to_dense: FxHashMap<u32, u32>,
    pub dense_to_orig: Vec<u32>,
    // used in reconstruct path after bfs
    pub redirects_passed: FxHashMap<(u32, u32), u32>,
}
impl CsrGraph {
    pub fn get(&self, dense_node: u32) -> &[u32] {
        let start = self.offsets[dense_node as usize] as usize;
        let end = self.offsets[dense_node as usize + 1] as usize;
        &self.edges[start..end]
    }
    pub fn get_reverse(&self, dense_node: u32) -> &[u32] {
        let start = self.reverse_offsets[dense_node as usize] as usize;
        let end = self.reverse_offsets[dense_node as usize + 1] as usize;
        &self.reverse_edges[start..end]
    }
}

// since some pages don't have links, those ids won't be in the adjacency list
// so need to base it off of id_to_title which has ids of all pages
pub fn build_csr_with_adjacency_list(
    id_to_title: &FxHashMap<u32, String>,
    adjacency_list: &FxHashMap<u32, Vec<u32>>,
    reverse_adjacency_list: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
) -> CsrGraph {
    // terminology: if apple -> banana, apple is the row title, banana is column
    let hasher = FxBuildHasher::default();
    let num_nodes = id_to_title.len();

    let mut orig_to_dense: FxHashMap<u32, u32> =
        FxHashMap::with_capacity_and_hasher(num_nodes, hasher);
    let mut dense_to_orig: Vec<u32> = Vec::with_capacity(num_nodes);

    for (i, row_title) in id_to_title.keys().enumerate() {
        orig_to_dense.insert(*row_title, i as u32);
        dense_to_orig.push(*row_title);
    }

    let mut offsets = Vec::with_capacity(num_nodes + 1);
    let mut reverse_offsets = Vec::with_capacity(num_nodes + 1);
    let mut edges = Vec::new();
    let mut reverse_edges = Vec::new();
    offsets.push(0);
    reverse_offsets.push(0);

    for row_title in &dense_to_orig {
        if let Some(neighbors) = adjacency_list.get(row_title) {
            let mut dense_neighbors: Vec<u32> = neighbors
                .iter()
                .filter_map(|to| orig_to_dense.get(to).copied())
                .collect();
            dense_neighbors.sort_unstable(); // for locality?
            edges.extend(dense_neighbors);
        }
        offsets.push(edges.len() as u32);

        // building reverse edges
        if let Some(neighbors) = reverse_adjacency_list.get(row_title) {
            let mut dense_neighbors: Vec<u32> = neighbors
                .iter()
                .filter_map(|to| orig_to_dense.get(to).copied())
                .collect();
            dense_neighbors.sort_unstable(); // for locality?
            reverse_edges.extend(dense_neighbors);
        }
        reverse_offsets.push(reverse_edges.len() as u32);
    }

    // translate redirect targets
    let mut redirects_passed_dense: FxHashMap<(u32, u32), u32> = FxHashMap::default();
    let mut skip_count = 0;
    for (&(from_orig, to_orig), &redirect_orig) in redirects_passed.iter() {
        if let (Some(&from_dense), Some(&to_dense), Some(&redirect_dense)) = (
            orig_to_dense.get(&from_orig),
            orig_to_dense.get(&to_orig),
            orig_to_dense.get(&redirect_orig),
        ) {
            redirects_passed_dense.insert((from_dense, to_dense), redirect_dense);
        } else {
            skip_count += 1;
        }
    }

    println!(
        "Skipped {} redirects that could not be translated to dense IDs",
        skip_count
    );

    println!(
        "redirect target dense len: {}, redirect targets len: {}",
        redirects_passed_dense.len(),
        redirects_passed.len()
    );
    CsrGraph {
        offsets,
        edges,
        reverse_offsets,
        reverse_edges,
        orig_to_dense,
        dense_to_orig,
        redirects_passed: redirects_passed_dense,
    }
}

// ugly parser 180s
// regex

pub fn build_pagelinks(
    path: &str,
    linktargets: &FxHashMap<u32, u32>,
    redirect_targets: &FxHashMap<u32, u32>,
) -> anyhow::Result<(
    FxHashMap<u32, Vec<u32>>,
    FxHashMap<u32, Vec<u32>>,
    FxHashMap<(u32, u32), u32>,
)> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let file_size = metadata.len();

    let buf_reader = BufReader::with_capacity(128 * 1024, file); // 128 KB buffer

    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )?
        .progress_chars("=>-"),
    );
    let pb_reader = pb.wrap_read(buf_reader);
    let decoder = GzDecoder::new(pb_reader);

    let mut decompressed_reader = BufReader::new(decoder);

    // (1939 -> page_id from,0,2 -> page_id to)
    // let tuple_re = Regex::new(r"\((\d+),0,(\d+)").unwrap();

    let estimated_entries = 19_000_000; // 18 570 593 number of pages?
                                        // 1 598 445 028 edges?
    let hasher = FxBuildHasher::default();
    let mut pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
        FxHashMap::with_capacity_and_hasher(estimated_entries, hasher);
    // if a link on a page is a redirect, store in the map[(page from, redirect target to), redirect to] so we can turn it back later
    let mut redirects_passed: FxHashMap<(u32, u32), u32> = FxHashMap::default();

    // regex too slow (10 mins, this is 4 min)
    // const PREFIX: &str = "INSERT INTO `pagelinks` VALUES (";

    let mut line_buf = Vec::new();

    let mut skip_count_ns = 0;
    while decompressed_reader.read_until(b'\n', &mut line_buf)? != 0 {
        parse_line_bytes(
            &line_buf,
            &linktargets,
            &redirect_targets,
            &mut pagelinks_adjacency_list,
            &mut redirects_passed,
            &mut skip_count_ns,
        );
        line_buf.clear();
    }

    println!("DEDUPING NEIGHBORS");
    for neighbors in pagelinks_adjacency_list.values_mut() {
        neighbors.sort_unstable();
        neighbors.dedup();
    }

    println!("page_links map length: {}", pagelinks_adjacency_list.len());
    println!("skipped ns: {}", skip_count_ns);

    println!("build incoming pagelinks");
    let mut incoming_pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> = FxHashMap::default();

    // could also build this alongside the regular pagelinks map, but this is more clear
    for (&from, to_list) in pagelinks_adjacency_list.iter() {
        for &to in to_list {
            incoming_pagelinks_adjacency_list
                .entry(to)
                .or_default()
                .push(from);
        }
    }

    Ok((
        pagelinks_adjacency_list,
        incoming_pagelinks_adjacency_list,
        redirects_passed,
    ))
}

// example data
// INSERT INTO `pagelinks` VALUES (1939,0,2),(3040,0,2),
fn parse_line_bytes(
    line_buf: &[u8],
    linktargets: &FxHashMap<u32, u32>,
    redirect_targets: &FxHashMap<u32, u32>,
    page_links: &mut FxHashMap<u32, Vec<u32>>,
    redirects_passed: &mut FxHashMap<(u32, u32), u32>,
    skip_count_ns: &mut usize,
) {
    const PREFIX: &[u8] = b"INSERT INTO `pagelinks` VALUES (";

    if !line_buf.starts_with(PREFIX) {
        return;
    }

    let mut i = PREFIX.len(); // start after the prefix
    let len = line_buf.len();

    while i < len {
        if line_buf[i] != b'(' {
            i += 1;
            continue;
        }

        i += 1; // skip '('
        let start = i;

        // -------- Field 1: from --------
        while i < len && line_buf[i] != b',' {
            i += 1;
        }
        let field1 = &line_buf[start..i];
        let page_id_from = atoi::atoi::<u32>(field1).unwrap_or_else(|| {
            panic!(
                "Invalid page_id_from: {:?}",
                std::str::from_utf8(field1).unwrap_or("<invalid>")
            )
        });

        i += 1; // skip ','

        // -------- Field 2: namespace --------
        let start = i;
        while i < len && line_buf[i] != b',' {
            i += 1;
        }
        let field2 = &line_buf[start..i];
        if field2 != b"0" {
            *skip_count_ns += 1;
            // skip this tuple
            while i < len && line_buf[i] != b')' {
                i += 1;
            }
            i += 1; // skip ')'
            if i < len && line_buf[i] == b',' {
                i += 1; // skip ',' after tuple
            }
            continue;
        }

        i += 1; // skip ','

        // -------- Field 3: to --------
        let start = i;
        while i < len && line_buf[i] != b')' {
            i += 1;
        }
        let field3 = &line_buf[start..i];
        let page_id_to = atoi::atoi::<u32>(field3).unwrap_or_else(|| {
            panic!(
                "Invalid page_id_to: {:?}",
                std::str::from_utf8(field3).unwrap_or("<invalid>")
            )
        });

        if let Some(mut mapped_target) = linktargets.get(&page_id_to) {
            // -------- Resolve redirect --------
            // i take it back im going to NUKE REDIRECTS
            // in the case a page has links to both a redirect and the redirect target,
            // i will keep only one (the redirect will be logged in redirects_passed so itll replace it with the redirect one)
            if let Some(redirect_target) = redirect_targets.get(mapped_target) {
                redirects_passed.insert((page_id_from, *redirect_target), *mapped_target);
                mapped_target = redirect_target;
            }
            page_links
                .entry(page_id_from)
                .or_default()
                .push(*mapped_target);
        }

        i += 1; // skip ')'
        if i < len && line_buf[i] == b',' {
            i += 1; // skip ',' after tuple
        }
    }
}
