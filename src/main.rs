mod page_parser;
mod redirect_parser;
mod util;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    time::Instant,
};

fn load_page_ids(path: &str) -> std::io::Result<Vec<u32>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut ids = Vec::new();
    for line in reader.lines() {
        if let Ok(line) = line {
            if let Ok(id) = line.trim().parse::<u32>() {
                ids.push(id);
            }
        }
    }
    Ok(ids)
}
pub fn benchmark_redirects(
    ids: &[u32],
    redirect_hashmap: &HashMap<u32, u32>,
    redirect_vec_map: &redirect_parser::RedirectVecMap,
) {
    // Benchmark HashMap
    let start = Instant::now();
    let mut count_hashmap = 0;
    for &id in ids {
        if redirect_hashmap.get(&id).is_some() {
            count_hashmap += 1;
        }
    }
    let hashmap_duration = start.elapsed();
    println!(
        "HashMap: Found {} redirects in {:?}",
        count_hashmap, hashmap_duration
    );

    // Benchmark RedirectVecMap (binary search)
    let start = Instant::now();
    let mut count_vecmap = 0;
    for &id in ids {
        if redirect_vec_map.get(id).is_some() {
            count_vecmap += 1;
        }
    }
    let vecmap_duration = start.elapsed();
    println!(
        "RedirectVecMap: Found {} redirects in {:?}",
        count_vecmap, vecmap_duration
    );
}

fn main() -> anyhow::Result<()> {
    let now = Instant::now();

    // let title_to_id: HashMap<String, u32> = util::load_from_file("data/title_to_id.bin")?;
    // let (redirect_vec_map, redirect_hashmap) = redirect_parser::build_redirect_targets(
    //     "../sql_files/enwiki-latest-redirect.sql.gz",
    //     &title_to_id,
    // )?;

    // util::save_to_file(&redirect_hashmap, "data/redirect_hashmap.bin")?;
    // util::save_to_file(&redirect_vec_map, "data/redirect_vec_map.bin")?;
    // let title_to_id: HashMap<String, u32> = util::load_from_file("data/title_to_id.bin")?;
    // let id_to_title: HashMap<u32, String> = util::load_from_file("data/id_to_title.bin")?;
    let redirect_hashmap: HashMap<u32, u32> = util::load_from_file("data/redirect_hashmap.bin")?;
    let redirect_vec_map: redirect_parser::RedirectVecMap =
        util::load_from_file("data/redirect_vec_map.bin")?;

    // util::run_interactive_session(
    //     &title_to_id,
    //     &id_to_title,
    //     &redirect_hashmap,
    //     &redirect_vec_map,
    // )?;

    let page_ids = load_page_ids("data/redirect_page_ids.txt")?;
    println!("{}", page_ids.len());
    benchmark_redirects(&page_ids, &redirect_hashmap, &redirect_vec_map);

    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);
    Ok(())
}
