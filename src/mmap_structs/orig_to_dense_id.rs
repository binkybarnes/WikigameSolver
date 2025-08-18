use crate::util;
use memmap2::Mmap;
use rustc_hash::FxHashMap;
use std::cmp::Ordering;

pub struct OrigToDenseIdMmap {
    pub orig_ids: Mmap,  // Vec<u32>, sorted original IDs
    pub dense_ids: Mmap, // Vec<u32>, parallel to orig_ids
}

impl OrigToDenseIdMmap {
    /// Given an original ID, return the dense ID via binary search
    pub fn get(&self, orig_id: u32) -> Option<u32> {
        let orig_ids: &[u32] = util::mmap_as_u32_slice(&self.orig_ids);
        let dense_ids: &[u32] = util::mmap_as_u32_slice(&self.dense_ids);

        let mut lo = 0usize;
        let mut hi = orig_ids.len();

        while lo < hi {
            let mid = (lo + hi) / 2;
            match orig_ids[mid].cmp(&orig_id) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(dense_ids[mid]),
            }
        }

        None
    }
}

/// Build the memory-mapped structure
pub fn build_and_save_orig_to_dense_id_mmap() -> anyhow::Result<()> {
    let orig_to_dense_id: FxHashMap<u32, u32> = util::load_from_file("data/orig_to_dense_id.bin")?;

    // Collect and sort by original ID
    let mut entries: Vec<(&u32, &u32)> = orig_to_dense_id.iter().collect();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));

    let mut orig_ids: Vec<u32> = Vec::with_capacity(entries.len());
    let mut dense_ids: Vec<u32> = Vec::with_capacity(entries.len());

    for (orig, dense) in entries {
        orig_ids.push(*orig);
        dense_ids.push(*dense);
    }

    // Save to disk
    util::write_u32_vec_to_file(&orig_ids, "data/orig_to_dense_id/orig_ids.bin")?;
    util::write_u32_vec_to_file(&dense_ids, "data/orig_to_dense_id/dense_ids.bin")?;

    Ok(())
}

/// Load the memory-mapped structure
pub fn load_orig_to_dense_id_mmap() -> anyhow::Result<OrigToDenseIdMmap> {
    let orig_ids = util::mmap_file("data/orig_to_dense_id/orig_ids.bin")?;
    let dense_ids = util::mmap_file("data/orig_to_dense_id/dense_ids.bin")?;

    Ok(OrigToDenseIdMmap {
        orig_ids,
        dense_ids,
    })
}
