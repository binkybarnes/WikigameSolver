use crate::pagelinks_parser;
use crate::redirect_parser;
use crate::search;
use bitcode::Decode;
use bitcode::Encode;
use bytemuck::cast_slice;
use memmap2::Mmap;
use rustc_hash::{FxBuildHasher, FxHashMap};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::create_dir_all;
use std::io::Read;
use std::path::Path;
use std::{
    fs::File,
    io::{self, BufReader, BufWriter, Write},
};

pub fn unescape_sql_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    '\\' => result.push('\\'),
                    '\'' => result.push('\''),
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    _ => {
                        // Unknown escape, keep both
                        result.push('\\');
                        result.push(next);
                    }
                }
            } else {
                result.push('\\');
            }
        } else {
            result.push(c);
        }
    }
    result
}

pub fn save_to_file<T: Encode>(data: &T, path: &str) -> anyhow::Result<()> {
    println!("Encoding and saving to file");
    let encoded = bitcode::encode(data); // Encode into Vec<u8>

    // Ensure the parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }

    let file = File::create(path)?;
    let mut writer = BufWriter::with_capacity(128 * 1024, file);

    writer.write_all(&encoded)?;
    writer.flush()?;

    Ok(())
}

pub fn load_from_file<T: for<'a> Decode<'a>>(path: &str) -> anyhow::Result<T> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(128 * 1024, file);

    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    let decoded =
        bitcode::decode(&buffer).map_err(|e| anyhow::anyhow!("bitcode decode error: {:?}", e))?;

    Ok(decoded)
}

/// Write a Vec<u32> as raw little-endian bytes to disk.
/// For portability/AArch64 differences you can write u32::to_le_bytes in a loop,
/// but this fast path assumes native little-endian.
pub fn write_u32_vec_to_file(v: &Vec<u32>, path: &str) -> anyhow::Result<()> {
    println!("Writing u32 and saving to file");

    // Ensure the parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }

    let mut file = File::create(path)?;
    // cast_slice is safe if u32 is Pod (it is) and we remain on same arch.
    let bytes: &[u8] = cast_slice(v.as_slice());
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

pub fn write_u8_vec_to_file(v: &Vec<u8>, path: &str) -> anyhow::Result<()> {
    println!("Writing u8 vector to file");

    // Ensure the parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }

    let mut file = File::create(path)?;
    file.write_all(v)?;
    file.sync_all()?;
    Ok(())
}

/// Memory-map file and return the Mmap object.
pub fn mmap_file(path: &str) -> anyhow::Result<Mmap> {
    let file = File::open(path)?;
    // Safety: mapping readonly file
    let mmap = unsafe { Mmap::map(&file)? };
    Ok(mmap)
}

/// Cast a Mmap (bytes) to &[u32]. Caller must ensure alignment and length multiple of 4.
/// This returns a runtime slice, not a stored reference into the struct.
pub fn mmap_as_u32_slice(mmap: &Mmap) -> &'_ [u32] {
    // bytemuck::cast_slice will panic if the length is not a multiple of u32
    bytemuck::cast_slice::<u8, u32>(&mmap[..])
}

// pub fn save_to_file<T: Serialize>(data: &T, path: &str) -> anyhow::Result<()> {
//     println!("Serializing and saving to file");
//     let file = File::create(path)?;
//     let writer = BufWriter::with_capacity(128 * 1024, file);
//     bincode::serialize_into(writer, data)?;

//     Ok(())
// }

// pub fn load_from_file<T: DeserializeOwned>(path: &str) -> anyhow::Result<T> {
//     let file = File::open(path)?;
//     let mut buf_reader = BufReader::with_capacity(128 * 1024, file); // 128 kib
//     let data: T = bincode::deserialize_from(&mut buf_reader)?;

//     Ok(data)
// }

// // TODO!
// pub fn run_interactive_session<G: pagelinks_parser::CsrGraphTrait>(
//     title_to_id: &FxHashMap<String, u32>,
//     id_to_title: &FxHashMap<u32, String>,
//     redirect_targets: &FxHashMap<u32, u32>,
//     linktargets: &FxHashMap<u32, u32>,
//     pagelinks_adjacency_list: &FxHashMap<u32, Vec<u32>>,
//     incoming_pagelinks_adjacency_list: &FxHashMap<u32, Vec<u32>>,
//     pagelinks_csr: &G,
// ) -> anyhow::Result<()> {
//     loop {
//         print!("> ");
//         io::stdout().flush()?;
//         let mut input = String::new();
//         io::stdin().read_line(&mut input)?;
//         let input = input.trim();

//         // Commands
//         if input == "exit" {
//             break;
//         } else if input.starts_with("lookup ") {
//             let title = input.strip_prefix("lookup ").unwrap();
//             match title_to_id.get(title) {
//                 Some(id) => println!("ID: {}", id),
//                 None => println!("Title not found"),
//             }
//         } else if input.starts_with("reverse ") {
//             let id = input.strip_prefix("reverse ").unwrap().parse::<u32>();
//             match id {
//                 Ok(id) => match id_to_title.get(&id) {
//                     Some(title) => println!("Title: {}", title),
//                     None => println!("ID not found"),
//                 },
//                 Err(_) => println!("Invalid ID"),
//             }
//         } else if input.starts_with("redirect ") {
//             let id = input.strip_prefix("redirect ").unwrap().parse::<u32>();
//             match id {
//                 Ok(source_id) => {
//                     let map_result = redirect_targets.get(&source_id);
//                     if let Some(target_id) = map_result {
//                         println!("Redirects to ID: {}", target_id);
//                     } else {
//                         println!("ID {} is not a redirect.", source_id);
//                     }
//                 }
//                 Err(_) => {
//                     println!("Invalid ID.");
//                 }
//             }
//         } else if input.starts_with("linktarget ") {
//             let id = input.strip_prefix("linktarget ").unwrap().parse::<u32>();
//             match id {
//                 Ok(linktarget_id) => {
//                     if let Some(target_id) = linktargets.get(&linktarget_id) {
//                         println!("linktarget_id {} -> target_id {}", linktarget_id, target_id);
//                     } else {
//                         println!("linktarget_id {} not found", linktarget_id);
//                     }
//                 }
//                 Err(_) => println!("Invalid linktarget_id."),
//             }
//         } else if input.starts_with("links ") {
//             let page_id_res = input.strip_prefix("links ").unwrap().parse::<u32>();
//             match page_id_res {
//                 Ok(page_id) => {
//                     // Print neighbors from hashmap adjacency list
//                     match pagelinks_adjacency_list.get(&page_id) {
//                         Some(neighbors) => {
//                             println!("HashMap neighbors ({}): {:?}", neighbors.len(), neighbors);
//                         }
//                         None => {
//                             println!("No neighbors found in HashMap for page ID {}", page_id);
//                         }
//                     }

//                     // Print neighbors from CSR graph if provided

//                     if let Some(&dense_idx) = pagelinks_csr.orig_to_dense().get(&page_id) {
//                         let dense_neighbors = pagelinks_csr.get(dense_idx);
//                         let orig_neighbors: Vec<u32> = dense_neighbors
//                             .iter()
//                             .map(|&dense_n| pagelinks_csr.dense_to_orig()[dense_n as usize])
//                             .collect();
//                         println!(
//                             "CSR neighbors ({}): {:?}",
//                             orig_neighbors.len(),
//                             orig_neighbors
//                         );
//                     } else {
//                         println!("Page ID {} not found in CSR graph", page_id);
//                     }
//                 }
//                 Err(_) => {
//                     println!("Invalid page ID.");
//                 }
//             }
//         } else if input.starts_with("incoming links ") {
//             let page_id_res = input
//                 .strip_prefix("incoming links ")
//                 .unwrap()
//                 .parse::<u32>();
//             match page_id_res {
//                 Ok(page_id) => {
//                     // Print neighbors from hashmap adjacency list
//                     match incoming_pagelinks_adjacency_list.get(&page_id) {
//                         Some(neighbors) => {
//                             println!("HashMap neighbors ({}): {:?}", neighbors.len(), neighbors);
//                         }
//                         None => {
//                             println!("No neighbors found in HashMap for page ID {}", page_id);
//                         }
//                     }

//                     // // Print neighbors from CSR graph if provided

//                     // if let Some(&dense_idx) = pagelinks_csr.orig_to_dense.get(&page_id) {
//                     //     let dense_neighbors = pagelinks_csr.get(dense_idx);
//                     //     let orig_neighbors: Vec<u32> = dense_neighbors
//                     //         .iter()
//                     //         .map(|&dense_n| pagelinks_csr.dense_to_orig[dense_n as usize])
//                     //         .collect();
//                     //     println!(
//                     //         "CSR neighbors ({}): {:?}",
//                     //         orig_neighbors.len(),
//                     //         orig_neighbors
//                     //     );
//                     // } else {
//                     //     println!("Page ID {} not found in CSR graph", page_id);
//                     // }
//                 }
//                 Err(_) => {
//                     println!("Invalid page ID.");
//                 }
//             }
//         }
//         // else if input.starts_with("search ") {
//         //     // Parse two arguments: start_id and goal_id
//         //     let args: Vec<&str> = input["search ".len()..].split_whitespace().collect();
//         //     if args.len() != 2 {
//         //         println!("Usage: search <start_id> <goal_id>");
//         //     } else {
//         //         let start_res = args[0].parse::<u32>();
//         //         let goal_res = args[1].parse::<u32>();

//         //         match (start_res, goal_res) {
//         //             (Ok(start), Ok(goal)) => {
//         //                 let max_depth = 7; // or some reasonable default or configurable value
//         //                 match search::bfs_adj_list(
//         //                     &pagelinks_adjacency_list,
//         //                     &redirect_targets,
//         //                     start,
//         //                     goal,
//         //                     max_depth,
//         //                 ) {
//         //                     Some(path) => {
//         //                         println!("Path found (length {}): {:?}", path.len(), path);
//         //                     }
//         //                     None => {
//         //                         println!(
//         //                             "No path found from {} to {} within depth {}",
//         //                             start, goal, max_depth
//         //                         );
//         //                     }
//         //                 }
//         //             }
//         //             _ => {
//         //                 println!("Invalid start or goal page ID.");
//         //             }
//         //         }
//         //     }
//         // }
//         else {
//             println!("Unknown command. Try: lookup <title>, reverse <id>, redirect <id>, linktargets <linktarget id>, links <page_id>, search <start_id> <goal_id>, exit");
//         }
//     }

//     Ok(())
// }
