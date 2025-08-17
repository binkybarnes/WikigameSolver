use crate::util;
use memmap2::Mmap;
use rustc_hash::FxHashMap;
use std::cmp::Ordering;

pub struct TitleToDenseIdMmap {
    // sorted so can perform binary search
    pub titles: Mmap,    // Vec<u8> blob of characters?
    pub offsets: Mmap,   // Vec<u32>
    pub dense_ids: Mmap, // Vec<u32>
}

impl TitleToDenseIdMmap {
    // given title, return dense id via binary search
    // courtesy of mister gippity
    pub fn get(&self, title: &str) -> Option<u32> {
        let offsets: &[u32] = util::mmap_as_u32_slice(&self.offsets);
        let dense_ids: &[u32] = util::mmap_as_u32_slice(&self.dense_ids);

        let bytes = &self.titles[..];
        let needle = title.as_bytes();

        let n = dense_ids.len(); // number of titles
        let mut lo = 0usize;
        let mut hi = n; // search in [lo, hi)

        while lo < hi {
            let mid = (lo + hi) / 2;
            let start = offsets[mid] as usize;
            // Support either offsets.len() == n or n+1 (with sentinel)
            let end = if mid + 1 < offsets.len() {
                offsets[mid + 1] as usize
            } else {
                bytes.len()
            };

            let s_bytes = &bytes[start..end];
            match s_bytes.cmp(needle) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(dense_ids[mid]),
            }
        }
        None
    }
}

pub fn build_and_save_title_to_dense_id_mmap() -> anyhow::Result<()> {
    let title_to_dense_id: FxHashMap<String, u32> =
        util::load_from_file("data/title_to_dense_id.bin")?;

    // Step 1: collect (title, dense_id) pairs and sort by title
    let mut entries: Vec<(&String, &u32)> = title_to_dense_id.iter().collect();
    entries.sort_by(|(a_title, _), (b_title, _)| a_title.cmp(b_title));

    // Step 2: build UTF-8 blob + offsets + dense_ids
    let mut titles_blob: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(entries.len());
    let mut dense_ids: Vec<u32> = Vec::with_capacity(entries.len());

    for (title, dense_id) in entries {
        offsets.push(titles_blob.len() as u32);
        titles_blob.extend_from_slice(title.as_bytes());
        dense_ids.push(*dense_id);
    }

    // Step 3: save
    util::write_u8_vec_to_file(&titles_blob, "data/title_to_dense_id/titles.bin")?;
    util::write_u32_vec_to_file(&offsets, "data/title_to_dense_id/offsets.bin")?;
    util::write_u32_vec_to_file(&dense_ids, "data/title_to_dense_id/dense_ids.bin")?;

    Ok(())
}

pub fn load_title_to_dense_id_mmap() -> anyhow::Result<TitleToDenseIdMmap> {
    let titles = util::mmap_file("data/title_to_dense_id/titles.bin")?;
    let offsets = util::mmap_file("data/title_to_dense_id/offsets.bin")?;
    let dense_ids = util::mmap_file("data/title_to_dense_id/dense_ids.bin")?;

    Ok(TitleToDenseIdMmap {
        titles,
        offsets,
        dense_ids,
    })
}
