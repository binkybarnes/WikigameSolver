use crate::util;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};

pub fn build_title_maps(
    path: &str,
) -> anyhow::Result<(FxHashMap<String, u32>, FxHashMap<u32, String>)> {
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

    // (10 -> page_id,0 -> main namespace,'Computer_accessibility' -> page_title
    let tuple_re = Regex::new(r"\((\d+),0,'((?:[^'\\]|\\.)*)'").unwrap();

    // regex progress bar
    let estimated_matches = 19_000_000; // 18 570 593
    let pb_parse = ProgressBar::new(estimated_matches as u64);
    pb_parse.set_style(
        ProgressStyle::with_template(
            "[parsing] [{elapsed_precise}] [{bar:40.green/blue}] {pos}/{len} ({eta})",
        )?
        .progress_chars("=>-"),
    );

    let hasher = FxBuildHasher::default();
    let mut title_to_id: FxHashMap<String, u32> =
        FxHashMap::with_capacity_and_hasher(estimated_matches, hasher);
    let mut id_to_title: FxHashMap<u32, String> =
        FxHashMap::with_capacity_and_hasher(estimated_matches, hasher);

    for cap in tuple_re.captures_iter(&decompressed) {
        let page_id: u32 = cap[1].parse()?;
        let raw_title = &cap[2];

        // Handle escaped stuff
        let unescaped_title = util::unescape_sql_string(&raw_title);

        title_to_id.insert(unescaped_title.clone(), page_id);
        id_to_title.insert(page_id, unescaped_title);
        pb_parse.inc(1);
    }

    println!("Total titles parsed: {}", title_to_id.len());

    Ok((title_to_id, id_to_title))
}

// use crate::util;
// use flate2::read::GzDecoder;
// use indicatif::{ProgressBar, ProgressStyle};
// use regex::Regex;
// use rustc_hash::{FxBuildHasher, FxHashMap};
// use std::fs::File;
// use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

// pub fn build_title_maps(
//     path: &str,
// ) -> anyhow::Result<(FxHashMap<String, u32>, FxHashMap<u32, String>)> {
//     let file = File::open(path)?;
//     let metadata = file.metadata()?;
//     let file_size = metadata.len();

//     let buf_reader = BufReader::with_capacity(128 * 1024, file); // 128 KB buffer

//     let pb = ProgressBar::new(file_size);
//     pb.set_style(
//         ProgressStyle::with_template(
//             "[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
//         )?
//         .progress_chars("=>-"),
//     );
//     let pb_reader = pb.wrap_read(buf_reader);
//     let decoder = GzDecoder::new(pb_reader);
//     let decompressed_reader = BufReader::with_capacity(128 * 1024, decoder);

//     // Read entire decompressed content into a String
//     // let mut decompressed = String::new();
//     // decoder.read_to_string(&mut decompressed)?;
//     // println!("Decompressed len {}", decompressed.len());

//     // (10 -> page_id,0 -> main namespace,'Computer_accessibility' -> page_title
//     let tuple_re = Regex::new(r"\((\d+),0,'((?:[^'\\]|\\.)*)'").unwrap();

//     let estimated_matches = 19_000_000; // 18 570 593

//     let hasher = FxBuildHasher::default();
//     let mut title_to_id: FxHashMap<String, u32> =
//         FxHashMap::with_capacity_and_hasher(estimated_matches, hasher);
//     let mut id_to_title: FxHashMap<u32, String> =
//         FxHashMap::with_capacity_and_hasher(estimated_matches, hasher);

//     const PREFIX: &str = "INSERT INTO `page` VALUES (";
//     for line in decompressed_reader.lines() {
//         let line = line?;
//         if line.starts_with(PREFIX) {
//             for cap in tuple_re.captures_iter(&line) {
//                 let page_id: u32 = cap[1].parse()?;
//                 let raw_title = &cap[2];

//                 // Handle escaped stuff
//                 let unescaped_title = util::unescape_sql_string(&raw_title);

//                 title_to_id.insert(unescaped_title.clone(), page_id);
//                 id_to_title.insert(page_id, unescaped_title);
//             }
//         }
//     }

//     println!("Total titles parsed: {}", title_to_id.len());

//     Ok((title_to_id, id_to_title))
// }
