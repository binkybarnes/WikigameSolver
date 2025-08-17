use crate::parsers::*;
use crate::util;
use rustc_hash::FxHashMap;

pub fn build_and_save_pagelinks_adj_list() -> anyhow::Result<()> {
    let linktargets_dense: FxHashMap<u32, u32> =
        util::load_from_file("data/linktargets_dense.bin")?;
    let redirect_targets_dense: FxHashMap<u32, u32> =
        util::load_from_file("data/redirect_targets_dense.bin")?;
    let orig_to_dense_id: FxHashMap<u32, u32> = util::load_from_file("data/orig_to_dense_id.bin")?;

    let (pagelinks_adjacency_list, incoming_pagelinks_adjacency_list, redirects_passed_dense): (
        FxHashMap<u32, Vec<u32>>,
        FxHashMap<u32, Vec<u32>>,
        FxHashMap<(u32, u32), u32>,
    ) = pagelinks_parser::build_pagelinks_dense(
        "../sql_files/enwiki-latest-pagelinks.sql.gz",
        &linktargets_dense,
        &redirect_targets_dense,
        &orig_to_dense_id,
    )?;

    util::save_to_file(
        &pagelinks_adjacency_list,
        "data/pagelinks_adjacency_list.bin",
    )?;
    util::save_to_file(
        &incoming_pagelinks_adjacency_list,
        "data/incoming_pagelinks_adjacency_list.bin",
    )?;
    util::save_to_file(&redirects_passed_dense, "data/redirects_passed_dense.bin")?;

    Ok(())
}
