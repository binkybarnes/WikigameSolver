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

    let decompressed_reader = BufReader::new(decoder);

    // (1939 -> page_id from,0,2 -> page_id to)
    // let tuple_re = Regex::new(r"\((\d+),0,(\d+)").unwrap();

    let estimated_entries = 19_000_000; // 18 570 593 number of pages?
                                        // 1 598 445 028 edges?
    let hasher = FxBuildHasher::default();
    let mut page_links: FxHashMap<u32, Vec<u32>> =
        FxHashMap::with_capacity_and_hasher(estimated_entries, hasher);

    // regex too slow (10 mins, this is 1 min)
    const PREFIX: &str = "INSERT INTO `pagelinks` VALUES (";
    for line in decompressed_reader.lines() {
        let line = line?;
        if line.starts_with(PREFIX) {
            let Some(start_paren) = line.find('(') else {
                continue;
            };
            let Some(end_paren) = line.find(')') else {
                continue;
            };

            let tuple_str = &line[start_paren + 1..end_paren];
            let tuple: Vec<&str> = tuple_str.split(',').collect();
            if tuple.len() != 3 {
                println!("{:?}", tuple);
                std::process::exit(1);
            }
            if tuple[1] != "0" {
                continue;
            }

            let Ok(page_id_from) = tuple[0].parse::<u32>() else {
                continue;
            };
            let Ok(mut page_id_to) = tuple[2].parse::<u32>() else {
                continue;
            };
            // if it's a redirect resolve it
            if let Some(&redirect_target) = redirect_targets.get(&page_id_to) {
                page_id_to = redirect_target;
            }
            if id_to_title.contains_key(&page_id_to) {
                page_links.entry(page_id_from).or_default().push(page_id_to)
            }
        }
    }

    Ok(page_links)
}
