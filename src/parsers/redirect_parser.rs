use crate::util;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustc_hash::{FxBuildHasher, FxHashMap};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

pub fn build_redirect_targets_dense(
    path: &str,
    title_to_dense_id: &FxHashMap<String, u32>,
    orig_to_dense_id: &FxHashMap<u32, u32>,
) -> anyhow::Result<FxHashMap<u32, u32>> {
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
    let decompressed_reader = BufReader::with_capacity(128 * 1024, decoder);

    // (10 -> page_id,0 -> main namespace,'Computer_accessibility' -> redirect_target
    let tuple_re = Regex::new(r"\((\d+),0,'((?:[^'\\]|\\.)*)'").unwrap();

    let estimated_matches = 12_000_000; // 11 879 716

    let hasher = FxBuildHasher::default();

    // max page id: 80605290, count: 11879716
    let mut redirect_targets_dense: FxHashMap<u32, u32> =
        FxHashMap::with_capacity_and_hasher(estimated_matches, hasher);

    // let mut skipped_ids = Vec::new();

    const PREFIX: &str = "INSERT INTO `redirect` VALUES (";
    for line in decompressed_reader.lines() {
        let line = line?;
        if line.starts_with(PREFIX) {
            for cap in tuple_re.captures_iter(&line) {
                let page_id: u32 = cap[1].parse()?;
                let dense_id = match orig_to_dense_id.get(&page_id) {
                    Some(&id) => id,
                    None => {
                        // skipped_ids.push(page_id);
                        continue; // Skip this iteration
                    }
                };

                let redirect_target_title = &cap[2];

                // Handle escaped stuff
                let unescaped_title = util::unescape_sql_string(&redirect_target_title);

                // skip non existent target_title, ex: Chubchik
                // maybe also skip non existent rd_from
                if let Some(&target_dense_id) = title_to_dense_id.get(&unescaped_title) {
                    redirect_targets_dense.insert(dense_id, target_dense_id);
                }
            }
        }
    }

    println!("Total redirects parsed: {}", redirect_targets_dense.len());

    // // After processing, save skipped IDs
    // let skipped_path = "data/skipped_ids.txt";
    // if let Some(parent) = std::path::Path::new(skipped_path).parent() {
    //     if !parent.as_os_str().is_empty() {
    //         std::fs::create_dir_all(parent)?;
    //     }
    // }
    // let mut file = std::fs::File::create(skipped_path)?;
    // for id in &skipped_ids {
    //     writeln!(file, "{}", id)?;
    // }
    // println!(
    //     "Skipped {} IDs (saved to {})",
    //     skipped_ids.len(),
    //     skipped_path
    // );

    Ok(redirect_targets_dense)
}
