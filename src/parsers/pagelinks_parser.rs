use crate::util;
use bincode::deserialize_from_custom;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustc_hash::{FxBuildHasher, FxHashMap};
use serde::{Deserialize, Serialize}; // For the channel

use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

#[derive(Serialize, Deserialize)]
pub struct CsrGraph {
    pub offsets: Vec<usize>,
    pub edges: Vec<u32>,
    // page_ids are sparse, so map each page_id to 1, 2, ...
    pub orig_to_dense: FxHashMap<u32, usize>,
    pub dense_to_orig: Vec<u32>,
}
impl CsrGraph {
    pub fn get(&self, dense_node: usize) -> &[u32] {
        let start = self.offsets[dense_node];
        let end = self.offsets[dense_node + 1];
        &self.edges[start..end]
    }

    pub fn get_by_orig(&self, orig_id: u32) -> Option<&[u32]> {
        self.orig_to_dense
            .get(&orig_id)
            .map(|&dense_idx| self.get(dense_idx))
    }
}
pub fn build_csr_with_adjacency_list(adjacency_list: &FxHashMap<u32, Vec<u32>>) -> CsrGraph {
    // terminology: if apple -> banana, apple is the row title, banana is column
    let hasher = FxBuildHasher::default();
    let num_nodes = adjacency_list.len();

    let mut orig_to_dense: FxHashMap<u32, usize> =
        FxHashMap::with_capacity_and_hasher(num_nodes, hasher);
    let mut dense_to_orig: Vec<u32> = Vec::with_capacity(num_nodes);

    for (i, row_title) in adjacency_list.keys().enumerate() {
        orig_to_dense.insert(*row_title, i);
        dense_to_orig.push(*row_title);
    }

    let mut offsets = Vec::with_capacity(num_nodes + 1);
    let mut edges = Vec::new();
    offsets.push(0);

    for row_title in &dense_to_orig {
        let neighbors = adjacency_list.get(row_title).unwrap();
        let mut dense_neighbors: Vec<u32> = neighbors
            .iter()
            .filter_map(|to| orig_to_dense.get(to).map(|v| *v as u32))
            .collect();
        dense_neighbors.sort_unstable(); // for locality?

        edges.extend(dense_neighbors);
        offsets.push(edges.len());
    }

    CsrGraph {
        offsets,
        edges,
        orig_to_dense,
        dense_to_orig,
    }
}

// ugly parser 180s
// regex

pub fn build_pagelinks(
    path: &str,
    linktargets: &FxHashMap<u32, u32>,
) -> anyhow::Result<(FxHashMap<u32, Vec<u32>>, CsrGraph)> {
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

    // regex too slow (10 mins, this is 1 min)
    // const PREFIX: &str = "INSERT INTO `pagelinks` VALUES (";

    let mut line_buf = Vec::new();

    let mut skip_count_ns = 0;
    while decompressed_reader.read_until(b'\n', &mut line_buf)? != 0 {
        parse_line_bytes(
            &line_buf,
            &linktargets,
            &mut pagelinks_adjacency_list,
            &mut skip_count_ns,
        );
        line_buf.clear();
    }

    // should be
    // page_links map length: 14267418
    // skipped ns: 789014935, skipped not found: 510806857
    println!("page_links map length: {}", pagelinks_adjacency_list.len());
    println!("skipped ns: {}", skip_count_ns);

    println!("building csr");
    let pagelinks_csr: CsrGraph = build_csr_with_adjacency_list(&pagelinks_adjacency_list);
    Ok((pagelinks_adjacency_list, pagelinks_csr))
}

// example data
// INSERT INTO `pagelinks` VALUES (1939,0,2),(3040,0,2),
fn parse_line_bytes(
    line_buf: &[u8],
    linktargets: &FxHashMap<u32, u32>,
    page_links: &mut FxHashMap<u32, Vec<u32>>,
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

        if let Some(mapped_target) = linktargets.get(&page_id_to) {
            // -------- Resolve redirect --------
            // im going to allow redirects in the adjacency list for 2 reasons:
            // 1. when i find a path, i want to know if a page is a redirect, and i can resolve while visiting the node during search
            // 2.
            // if let Some(redirect_target) = redirect_targets.get(mapped_target) {
            //     mapped_target = redirect_target;
            // }
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
