use crate::util;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustc_hash::{FxBuildHasher, FxHashMap};
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

pub fn build_pagelinks(
    path: &str,
    id_to_title: &FxHashMap<u32, String>,
    redirect_targets: &FxHashMap<u32, u32>,
) -> anyhow::Result<(FxHashMap<u32, Vec<u32>>)> {
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
    let mut page_links: FxHashMap<u32, Vec<u32>> =
        FxHashMap::with_capacity_and_hasher(estimated_entries, hasher);

    // regex too slow (10 mins, this is 1 min)
    // const PREFIX: &str = "INSERT INTO `pagelinks` VALUES (";

    let mut line_buf = Vec::new();

    while decompressed_reader.read_until(b'\n', &mut line_buf)? != 0 {
        parse_line_bytes(&line_buf, &redirect_targets, &id_to_title, &mut page_links);
        line_buf.clear();
    }

    println!("Total links parsed: {}", page_links.len());
    Ok(page_links)
}

fn parse_line_bytes(
    line_buf: &[u8],
    redirect_targets: &FxHashMap<u32, u32>,
    id_to_title: &FxHashMap<u32, String>,
    page_links: &mut FxHashMap<u32, Vec<u32>>,
) {
    const PREFIX: &[u8] = b"INSERT INTO `pagelinks` VALUES (";
    let Some(mut tuples_part) = line_buf.strip_prefix(PREFIX) else {
        return;
    };

    // Loop through the "(from,ns,to)" tuples
    while !tuples_part.is_empty() {
        // Find the start and end of the tuple
        let Some(start_paren) = memchr::memchr(b'(', tuples_part) else {
            break;
        };
        let Some(end_paren) = memchr::memchr(b')', &tuples_part[start_paren..]) else {
            break;
        };
        let end_paren_abs = start_paren + end_paren;

        let mut fields = tuples_part[start_paren + 1..end_paren_abs].split(|&b| b == b',');

        // This is a much faster way to parse the 3 parts
        if let (Some(from_s), Some(ns_s), Some(to_s)) =
            (fields.next(), fields.next(), fields.next())
        {
            if ns_s == b"0" {
                if let (Some(page_id_from), Some(mut page_id_to)) =
                    (atoi::atoi(from_s), atoi::atoi(to_s))
                {
                    // -------- Resolve redirect --------
                    if let Some(&redirect_target) = redirect_targets.get(&page_id_to) {
                        page_id_to = redirect_target;
                    }

                    if id_to_title.contains_key(&page_id_to) {
                        page_links.entry(page_id_from).or_default().push(page_id_to);
                    }
                }
            }
        }

        // Move slice to the start of the next tuple
        tuples_part = &tuples_part[end_paren_abs + 1..];
    }
}

// // example data
// // INSERT INTO `pagelinks` VALUES (1939,0,2),(3040,0,2),
// fn parse_line_bytes(
//     line_buf: &[u8],
//     redirect_targets: &FxHashMap<u32, u32>,
//     id_to_title: &FxHashMap<u32, String>,
//     page_links: &mut FxHashMap<u32, Vec<u32>>,
// ) {
//     const PREFIX: &[u8] = b"INSERT INTO `pagelinks` VALUES (";

//     if !line_buf.starts_with(PREFIX) {
//         return;
//     }

//     let mut i = PREFIX.len(); // start after the prefix
//     let len = line_buf.len();

//     while i < len {
//         if line_buf[i] != b'(' {
//             i += 1;
//             continue;
//         }

//         i += 1; // skip '('
//         let start = i;

//         // -------- Field 1: from --------
//         while i < len && line_buf[i] != b',' {
//             i += 1;
//         }
//         let field1 = &line_buf[start..i];
//         let page_id_from = atoi::atoi::<u32>(field1).unwrap_or_else(|| {
//             panic!(
//                 "Invalid page_id_from: {:?}",
//                 std::str::from_utf8(field1).unwrap_or("<invalid>")
//             )
//         });

//         i += 1; // skip ','

//         // -------- Field 2: namespace --------
//         let start = i;
//         while i < len && line_buf[i] != b',' {
//             i += 1;
//         }
//         let field2 = &line_buf[start..i];
//         if field2 != b"0" {
//             // skip this tuple
//             while i < len && line_buf[i] != b')' {
//                 i += 1;
//             }
//             i += 1; // skip ')'
//             if i < len && line_buf[i] == b',' {
//                 i += 1; // skip ',' after tuple
//             }
//             continue;
//         }

//         i += 1; // skip ','

//         // -------- Field 3: to --------
//         let start = i;
//         while i < len && line_buf[i] != b')' {
//             i += 1;
//         }
//         let field3 = &line_buf[start..i];
//         let mut page_id_to = atoi::atoi::<u32>(field3).unwrap_or_else(|| {
//             panic!(
//                 "Invalid page_id_to: {:?}",
//                 std::str::from_utf8(field3).unwrap_or("<invalid>")
//             )
//         });

//         // -------- Resolve redirect --------
//         if let Some(&redirect_target) = redirect_targets.get(&page_id_to) {
//             page_id_to = redirect_target;
//         }

//         if id_to_title.contains_key(&page_id_to) {
//             page_links.entry(page_id_from).or_default().push(page_id_to);
//         }

//         i += 1; // skip ')'
//         if i < len && line_buf[i] == b',' {
//             i += 1; // skip ',' after tuple
//         }
//     }
// }

// 280s
// flate2 zlib-rs 250s
// byte parser 140s
// byte parser with memchar 147s
