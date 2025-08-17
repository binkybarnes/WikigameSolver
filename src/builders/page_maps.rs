use crate::parsers::*;
use crate::util;
use rustc_hash::FxHashMap;

pub fn build_and_save_page_maps_dense() -> anyhow::Result<()> {
    let (orig_to_dense_id, dense_id_to_orig, title_to_dense_id, dense_id_to_title): (
        FxHashMap<u32, u32>,    // orig_to_dense_id
        Vec<u32>,               // dense_id_to_orig
        FxHashMap<String, u32>, // title_to_dense_id
        Vec<String>,            // dense_id_to_title
    ) = page_parser::build_title_maps_dense("../sql_files/enwiki-latest-page.sql.gz")?;

    util::save_to_file(&orig_to_dense_id, "data/orig_to_dense_id.bin")?;
    util::save_to_file(&dense_id_to_orig, "data/dense_id_to_orig.bin")?;
    util::save_to_file(&title_to_dense_id, "data/title_to_dense_id.bin")?;
    util::save_to_file(&dense_id_to_title, "data/dense_id_to_title.bin")?;

    Ok(())
}
