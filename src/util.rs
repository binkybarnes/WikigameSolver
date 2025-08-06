use crate::redirect_parser;
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufReader, BufWriter, Write},
};

use serde::{de::DeserializeOwned, Serialize};

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

pub fn save_to_file<T: Serialize>(data: &T, path: &str) -> anyhow::Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::with_capacity(128 * 1024, file);
    bincode::serialize_into(writer, data)?;

    Ok(())
}

pub fn load_from_file<T: DeserializeOwned>(path: &str) -> anyhow::Result<T> {
    let file = File::open(path)?;
    let mut buf_reader = BufReader::with_capacity(128 * 1024, file); // 128 kib
    let data: T = bincode::deserialize_from(&mut buf_reader)?;

    Ok(data)
}

// TODO!
pub fn run_interactive_session(
    title_to_id: &HashMap<String, u32>,
    id_to_title: &HashMap<u32, String>,
    redirect_hashmap: &HashMap<u32, u32>,
    redirect_vec_map: &redirect_parser::RedirectVecMap,
) -> anyhow::Result<()> {
    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        // Commands
        if input == "exit" {
            break;
        } else if input.starts_with("lookup ") {
            let title = input.strip_prefix("lookup ").unwrap();
            match title_to_id.get(title) {
                Some(id) => println!("ID: {}", id),
                None => println!("Title not found"),
            }
        } else if input.starts_with("reverse ") {
            let id = input.strip_prefix("reverse ").unwrap().parse::<u32>();
            match id {
                Ok(id) => match id_to_title.get(&id) {
                    Some(title) => println!("Title: {}", title),
                    None => println!("ID not found"),
                },
                Err(_) => println!("Invalid ID"),
            }
        } else if input.starts_with("redirect ") {
            let id = input.strip_prefix("redirect ").unwrap().parse::<u32>();
            match id {
                Ok(source_id) => {
                    // Check hashmap
                    let map_result = redirect_hashmap.get(&source_id);
                    if let Some(target_id) = map_result {
                        println!("[HashMap] Redirects to ID: {}", target_id);
                    } else {
                        println!("[HashMap] ID {} is not a redirect.", source_id);
                    }

                    // Check vecmap
                    let vec_result = redirect_vec_map.get(source_id);
                    if let Some(target_id) = vec_result {
                        println!("[VecMap]   Redirects to ID: {}", target_id);
                    } else {
                        println!("[VecMap]   ID {} is not a redirect.", source_id);
                    }
                }
                Err(_) => {
                    println!("Invalid ID.");
                }
            }
        } else {
            println!("Unknown command. Try: lookup <title>, reverse <id>, redirect <id>, exit");
        }
    }

    Ok(())
}
