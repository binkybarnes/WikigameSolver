use crate::util;
use memmap2::Mmap;

pub struct RedirectTargetsDenseMmap {
    pub mmap: Mmap, // raw bytes
                    // pub len: usize, // number of u32 elements
}

impl RedirectTargetsDenseMmap {
    /// Get the target for a dense_id, returns u32::MAX if no redirect
    pub fn get(&self, dense_id: u32) -> u32 {
        // if dense_id as usize >= self.len {
        //     return u32::MAX;
        // }
        let start = dense_id as usize * 4;
        let end = start + 4;
        let bytes = &self.mmap[start..end];
        u32::from_le_bytes(bytes.try_into().unwrap())
    }
}

pub fn build_and_save_redirect_targets_dense_mmap() -> anyhow::Result<()> {
    let redirect_targets_dense: Vec<u32> = util::load_from_file("data/redirect_targets_dense.bin")?;
    util::write_u32_vec_to_file(
        &redirect_targets_dense,
        "data/redirect_targets_dense/redirect_targets_dense.bin",
    )?;
    Ok(())
}

pub fn load_redirect_targets_dense_mmap() -> anyhow::Result<RedirectTargetsDenseMmap> {
    let mmap: Mmap = util::mmap_file("data/redirect_targets_dense/redirect_targets_dense.bin")?;
    // let len = mmap.len() / 4; // number of u32 elements
    Ok(RedirectTargetsDenseMmap { mmap })
}
