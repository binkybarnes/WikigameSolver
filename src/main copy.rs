use flate2::read::GzDecoder;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::FxHashMap;
use std::fs::File;
use std::io::{BufReader, Read};

#[derive(Serialize, Deserialize)]
struct TitleIdMaps {
    title_to_id: FxHashMap<String, u32>,
    id_to_title: FxHashMap<u32, String>,
}

fn main() -> anyhow::Result<()> {
    let file = File::open("../sql_files/enwiki-latest-page.sql.gz")?;
    let decoder = GzDecoder::new(file);
    let mut reader = BufReader::new(decoder);

    // (10 <- page_id,0 <- article namespace,'AccessibleComputing' <- page title, ... )
    let tuple_re = Regex::new(r"\((\d+),0,'([^']*)'").unwrap();

    let mut title_to_id: FxHashMap<String, u32> = FxHashMap::new();
    let mut id_to_title: FxHashMap<u32, String> = FxHashMap::new();

    let mut buffer = String::new();
    let mut chunk = [0u8; 1_048_576]; // 1MB buffer

    let mut found_insert = false;

    loop {
        let n = reader.read(&mut chunk)?; // # of bytes read
        if n == 0 {
            break;
        }

        buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));

        if !found_insert {
            if let Some(pos) = buffer.find("INSERT INTO `page` VALUES ") {
                found_insert = true;
                // keep the stuff after VALUES
                buffer = buffer[pos + "INSERT INTO `page` VALUES ".len()..].to_string()
            } else {
                continue;
            }
        }

        // find last tuple end ),
        // split the buffer there and prepend it to the next chunk
        let last_tuple_end_pos = buffer.rfind("),");

        if let Some(pos) = last_tuple_end_pos {
            // pos points to ), we want to process up to pos+1 to include )

            let complete_part = &buffer[..pos + 1];
            let leftover = &buffer[pos + 2..]; // skip ),

            for cap in tuple_re.captures_iter(complete_part) {
                let page_id: u32 = cap[1].parse().unwrap();
                let page_title: String = cap[2].to_string();

                title_to_id.insert(page_title.clone(), page_id);
                id_to_title.insert(page_id, page_title);
            }

            buffer = leftover.to_string();
        }
    }

    // leftover tuples
    if found_insert {}

    Ok(())
}
