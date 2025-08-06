use crate::util;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};

pub fn build_redirect_targets(
    path: &str,
    title_to_id: &HashMap<String, u32>,
) -> anyhow::Result<(RedirectVecMap, HashMap<u32, u32>)> {
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
    let mut decoder = GzDecoder::new(pb_reader);

    // Read entire decompressed content into a String
    let mut decompressed = String::new();
    decoder.read_to_string(&mut decompressed)?;
    println!("Decompressed len {}", decompressed.len());

    // (10 -> page_id,0 -> main namespace,'Computer_accessibility' -> redirect_target
    let tuple_re = Regex::new(r"\((\d+),0,'((?:[^'\\]|\\.)*)'").unwrap();

    // regex progress bar
    let estimated_matches = 12_000_000; // 11 879 716
    let pb_parse = ProgressBar::new(estimated_matches as u64);
    pb_parse.set_style(
        ProgressStyle::with_template(
            "[parsing] [{elapsed_precise}] [{bar:40.green/blue}] {pos}/{len} ({eta})",
        )?
        .progress_chars("=>-"),
    );

    // max page id: 80605290, count: 11879716
    let mut redirect_pairs: Vec<(u32, u32)> = Vec::with_capacity(estimated_matches); // 11 879 716
    let mut redirect_hashmap: HashMap<u32, u32> = HashMap::with_capacity(estimated_matches);

    for cap in tuple_re.captures_iter(&decompressed) {
        let page_id: u32 = cap[1].parse()?;
        let redirect_target_title = &cap[2];

        // Handle escaped stuff
        let unescaped_title = util::unescape_sql_string(&redirect_target_title);

        // skip non existent target_title, ex: Chubchik
        if let Some(&target_id) = title_to_id.get(&unescaped_title) {
            redirect_pairs.push((page_id, target_id));
            redirect_hashmap.insert(page_id, target_id);
        }

        // redirect_targets[page_id] = title_to_id.get(&unescaped_title).copied();
        pb_parse.inc(1);
    }
    let redirect_vec_map = RedirectVecMap::new(redirect_pairs);

    println!("Total redirects parsed: {}", redirect_vec_map.len());

    Ok((redirect_vec_map, redirect_hashmap))
}

#[derive(Serialize, Deserialize)]
pub struct RedirectVecMap {
    redirects: Vec<(u32, u32)>,
}

impl RedirectVecMap {
    pub fn new(mut pairs: Vec<(u32, u32)>) -> Self {
        pairs.sort_unstable_by_key(|k| k.0);
        RedirectVecMap { redirects: pairs }
    }

    pub fn get(&self, page_id: u32) -> Option<u32> {
        self.redirects
            .binary_search_by_key(&page_id, |(from, _to)| *from)
            .ok()
            .map(|index| self.redirects[index].1)
    }

    pub fn len(&self) -> usize {
        self.redirects.len()
    }
}
