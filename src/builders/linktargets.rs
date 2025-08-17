use crate::parsers::*;
use crate::util;
use rustc_hash::FxHashMap;

pub fn build_and_save_linktargets_dense() -> anyhow::Result<()> {
    let title_to_id: FxHashMap<String, u32> = util::load_from_file("data/title_to_dense_id.bin")?;

    let linktargets_dense: FxHashMap<u32, u32> =
        build_linktargets_dense("../sql_files/enwiki-latest-linktarget.sql.gz", &title_to_id)?;

    util::save_to_file(&linktargets_dense, "data/linktargets_dense.bin")?;

    Ok(())
}
