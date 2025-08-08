use crate::util;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustc_hash::{FxBuildHasher, FxHashMap};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

// basically same as redirect_parser::build_redirect_targets
// because pl_target_id in pagelinks uses the id in linktargets
pub fn build_linktargets(
    path: &str,
    title_to_id: &FxHashMap<String, u32>,
) -> anyhow::Result<(FxHashMap<u32, u32>)> {
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
    let mut linktargets: FxHashMap<u32, u32> =
        FxHashMap::with_capacity_and_hasher(estimated_matches, hasher);

    const PREFIX: &str = "INSERT INTO `linktarget` VALUES (";
    for line in decompressed_reader.lines() {
        let line = line?;
        if line.starts_with(PREFIX) {
            for cap in tuple_re.captures_iter(&line) {
                let linktarget_id: u32 = cap[1].parse()?;
                let linktarget_title = &cap[2];

                // Handle escaped stuff
                let unescaped_title = util::unescape_sql_string(&linktarget_title);

                // skip non existent target_title, ex: Chubchik
                if let Some(&target_id) = title_to_id.get(&unescaped_title) {
                    linktargets.insert(linktarget_id, target_id);
                }
            }
        }
    }

    println!("Total linktargets parsed: {}", linktargets.len());

    Ok(linktargets)
}
