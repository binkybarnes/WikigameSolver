use crate::util;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

pub fn build_title_maps_dense(
    path: &str,
) -> anyhow::Result<(
    FxHashMap<u32, u32>,    // orig_to_dense_id
    Vec<u32>,               // dense_id_to_orig
    FxHashMap<String, u32>, // title_to_dense_id
    Vec<String>,            // dense_id_to_title
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
    let decompressed_reader = BufReader::with_capacity(128 * 1024, decoder);

    // (10 -> page_id,0 -> main namespace,'Computer_accessibility' -> page_title
    let tuple_re = Regex::new(r"\((\d+),0,'((?:[^'\\]|\\.)*)'").unwrap();

    let estimated_matches = 19_000_000; // 18 570 593

    let mut title_to_orig_id: FxHashMap<String, u32> =
        FxHashMap::with_capacity_and_hasher(estimated_matches, FxBuildHasher);
    let mut orig_id_to_title: FxHashMap<u32, String> =
        FxHashMap::with_capacity_and_hasher(estimated_matches, FxBuildHasher);

    const PREFIX: &str = "INSERT INTO `page` VALUES (";
    for line in decompressed_reader.lines() {
        let line = line?;
        if line.starts_with(PREFIX) {
            for cap in tuple_re.captures_iter(&line) {
                let page_id: u32 = cap[1].parse()?;
                if page_id == 53251 {
                    println!("found 53251");
                }
                let raw_title = &cap[2];

                // Handle escaped stuff
                let unescaped_title = util::unescape_sql_string(&raw_title);

                title_to_orig_id.insert(unescaped_title.clone(), page_id);
                orig_id_to_title.insert(page_id, unescaped_title);
            }
        }
    }

    println!("Total titles parsed: {}", title_to_orig_id.len());

    println!("building dense id maps");
    let num_nodes = orig_id_to_title.len();
    let mut orig_to_dense_id: FxHashMap<u32, u32> =
        FxHashMap::with_capacity_and_hasher(num_nodes, FxBuildHasher);
    let mut dense_id_to_orig: Vec<u32> = Vec::with_capacity(num_nodes);

    for (i, orig_id) in orig_id_to_title.keys().enumerate() {
        orig_to_dense_id.insert(*orig_id, i as u32);
        dense_id_to_orig.push(*orig_id);
    }

    println!("Converting title_to_id and id_to_title to dense ids");
    let mut dense_title_to_id: FxHashMap<String, u32> =
        FxHashMap::with_capacity_and_hasher(num_nodes, FxBuildHasher);
    let mut dense_id_to_title: Vec<String> = Vec::with_capacity(num_nodes);

    for (title, orig_id) in title_to_orig_id.into_iter() {
        if let Some(&dense_id) = orig_to_dense_id.get(&orig_id) {
            dense_title_to_id.insert(title.clone(), dense_id);
        }
    }

    for (orig_id, title) in orig_id_to_title.into_iter() {
        if let Some(&dense_id) = orig_to_dense_id.get(&orig_id) {
            dense_id_to_title.insert(dense_id as usize, title.clone());
        }
    }

    Ok((
        orig_to_dense_id,
        dense_id_to_orig,
        dense_title_to_id,
        dense_id_to_title,
    ))
}
