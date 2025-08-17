use crate::parsers::*;
use crate::util;
use rustc_hash::FxHashMap;

pub fn build_and_save_redirect_targets_dense() -> anyhow::Result<()> {
    let title_to_dense_id: FxHashMap<String, u32> =
        util::load_from_file("data/title_to_dense_id.bin")?;
    let orig_to_dense_id: FxHashMap<u32, u32> = util::load_from_file("data/orig_to_dense_id.bin")?;

    let redirect_targets_dense: Vec<u32> = build_redirect_targets_dense(
        "../sql_files/enwiki-latest-redirect.sql.gz",
        &title_to_dense_id,
        &orig_to_dense_id,
    )?;

    util::save_to_file(&redirect_targets_dense, "data/redirect_targets_dense.bin")?;

    Ok(())
}
