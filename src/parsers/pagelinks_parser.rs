use crate::util;
use bitcode::{Decode, Encode};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use memmap2::Mmap;
use regex::Regex;
use rustc_hash::{FxBuildHasher, FxHashMap};

use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

pub fn build_pagelinks_dense(
    path: &str,
    linktargets: &FxHashMap<u32, u32>,
    redirect_targets: &FxHashMap<u32, u32>,
    orig_to_dense_id: &FxHashMap<u32, u32>,
) -> anyhow::Result<(
    FxHashMap<u32, Vec<u32>>,   // adj list forward
    FxHashMap<u32, Vec<u32>>,   // adj list backward
    FxHashMap<(u32, u32), u32>, // redirects passed
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
    let mut pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
        FxHashMap::with_capacity_and_hasher(estimated_entries, FxBuildHasher);
    // if a link on a page is a redirect, store in the map[(page from, redirect target to), redirect to] so we can turn it back later
    let mut redirects_passed: FxHashMap<(u32, u32), u32> = FxHashMap::default();

    // regex too slow (10 mins, this is 4 min)
    // const PREFIX: &str = "INSERT INTO `pagelinks` VALUES (";

    let mut line_buf = Vec::new();

    let mut skip_count_ns = 0;
    while decompressed_reader.read_until(b'\n', &mut line_buf)? != 0 {
        parse_line_bytes(
            &line_buf,
            linktargets,
            redirect_targets,
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

    println!("Translating pagelinks adj list page from to dense id");
    let mut pagelinks_adjacency_list_dense: FxHashMap<u32, Vec<u32>> =
        FxHashMap::with_capacity_and_hasher(estimated_entries, FxBuildHasher);
    // linktargets already made the neighbors dense
    for (orig_from, neighbors_dense) in pagelinks_adjacency_list.into_iter() {
        if let Some(&dense_from) = orig_to_dense_id.get(&orig_from) {
            // neighbors are already dense, just copy them over
            pagelinks_adjacency_list_dense.insert(dense_from, neighbors_dense);
        }
    }

    println!("Translating redirects_passed from page to dense id");

    let mut redirects_passed_dense: FxHashMap<(u32, u32), u32> =
        FxHashMap::with_capacity_and_hasher(redirects_passed.len(), FxBuildHasher);

    for ((orig_from, redirect_target), redirect) in redirects_passed.into_iter() {
        if let Some(&dense_from) = orig_to_dense_id.get(&orig_from) {
            redirects_passed_dense.insert((dense_from, redirect_target), redirect);
        }
    }

    println!(
        "pagelinks_adjacency_list_dense length: {}",
        pagelinks_adjacency_list_dense.len()
    );

    println!("skipped ns: {}", skip_count_ns);

    println!("build incoming pagelinks");
    let mut incoming_pagelinks_adjacency_list_dense: FxHashMap<u32, Vec<u32>> =
        FxHashMap::default();

    // could also build this alongside the regular pagelinks map, but this is more clear
    for (&from, to_list) in pagelinks_adjacency_list_dense.iter() {
        for &to in to_list {
            incoming_pagelinks_adjacency_list_dense
                .entry(to)
                .or_default()
                .push(from);
        }
    }

    Ok((
        pagelinks_adjacency_list_dense,
        incoming_pagelinks_adjacency_list_dense,
        redirects_passed_dense,
    ))
}

// example data
// INSERT INTO `pagelinks` VALUES (1939,0,2),(3040,0,2),
fn parse_line_bytes(
    line_buf: &[u8],
    linktargets_dense: &FxHashMap<u32, u32>,
    redirect_targets_dense: &FxHashMap<u32, u32>,
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
        let page_id_to_linktarget = atoi::atoi::<u32>(field3).unwrap_or_else(|| {
            panic!(
                "Invalid page_id_to: {:?}",
                std::str::from_utf8(field3).unwrap_or("<invalid>")
            )
        });

        if let Some(mut dense_id_to) = linktargets_dense.get(&page_id_to_linktarget) {
            // -------- Resolve redirect --------
            // i take it back im going to NUKE REDIRECTS
            // in the case a page has links to both a redirect and the redirect target,
            // i will keep only one (the redirect will be logged in redirects_passed so itll replace it with the redirect one)
            if let Some(redirect_target) = redirect_targets_dense.get(dense_id_to) {
                redirects_passed.insert((page_id_from, *redirect_target), *dense_id_to);
                dense_id_to = redirect_target;
            }
            page_links
                .entry(page_id_from)
                .or_default()
                .push(*dense_id_to);
        }

        i += 1; // skip ')'
        if i < len && line_buf[i] == b',' {
            i += 1; // skip ',' after tuple
        }
    }
}
