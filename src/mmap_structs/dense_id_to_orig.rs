use crate::util;
use memmap2::Mmap;

pub struct DenseIdToOrigMmap {
    pub orig_ids: Mmap, // Vec<u32>
}

impl DenseIdToOrigMmap {
    pub fn get(&self, dense_id: u32) -> u32 {
        let orig_ids: &[u32] = util::mmap_as_u32_slice(&self.orig_ids);
        orig_ids[dense_id as usize]
    }
}

pub fn build_and_save_dense_id_to_orig_mmap() -> anyhow::Result<()> {
    let dense_id_to_orig: Vec<u32> = util::load_from_file("data/dense_id_to_orig.bin")?;

    // Save to disk as raw u32 bytes
    util::write_u32_vec_to_file(&dense_id_to_orig, "data/dense_id_to_orig/mmap.bin")?;

    Ok(())
}

pub fn load_dense_id_to_orig_mmap() -> anyhow::Result<DenseIdToOrigMmap> {
    let orig_ids: Mmap = util::mmap_file("data/dense_id_to_orig/mmap.bin")?;
    Ok(DenseIdToOrigMmap { orig_ids })
}
