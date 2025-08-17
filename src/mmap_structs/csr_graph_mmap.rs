use memmap2::Mmap;
use rustc_hash::FxHashMap;

use crate::graph::*;
use crate::util;

pub fn build_and_save_pagelinks_csr() -> anyhow::Result<()> {
    let pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
        util::load_from_file("data/pagelinks_adjacency_list.bin")?;
    let incoming_pagelinks_adjacency_list: FxHashMap<u32, Vec<u32>> =
        util::load_from_file("data/incoming_pagelinks_adjacency_list.bin")?;
    let orig_to_dense_id: FxHashMap<u32, u32> = util::load_from_file("data/orig_to_dense_id.bin")?;

    println!("building csr");
    let pagelinks_csr: CsrGraph = build_csr_with_adjacency_list(
        &orig_to_dense_id,
        &pagelinks_adjacency_list,
        &incoming_pagelinks_adjacency_list,
    );

    drop(pagelinks_adjacency_list);
    drop(incoming_pagelinks_adjacency_list);
    drop(orig_to_dense_id);

    // in memory version
    util::save_to_file(&pagelinks_csr, "data/pagelinks_csr.bin")?;

    // memory mappable version
    util::write_u32_vec_to_file(&pagelinks_csr.edges, "data/csr/edges.bin")?;
    util::write_u32_vec_to_file(&pagelinks_csr.reverse_edges, "data/csr/reverse_edges.bin")?;

    util::write_u32_vec_to_file(&pagelinks_csr.offsets, "data/csr/offsets.bin")?;
    util::write_u32_vec_to_file(
        &pagelinks_csr.reverse_offsets,
        "data/csr/reverse_offsets.bin",
    )?;

    Ok(())
}

pub fn load_csr_graph_mmap() -> anyhow::Result<CsrGraphMmap> {
    // Memory-map the big edge arrays
    let edges_mmap: Mmap = util::mmap_file("data/csr/edges.bin")?;
    let reverse_edges_mmap: Mmap = util::mmap_file("data/csr/reverse_edges.bin")?;

    let offsets: Mmap = util::mmap_file("data/csr/offsets.bin")?;
    let reverse_offsets: Mmap = util::mmap_file("data/csr/reverse_offsets.bin")?;

    Ok(CsrGraphMmap {
        offsets,
        reverse_offsets,
        edges_mmap,
        reverse_edges_mmap,
    })
}
