use crate::util;
use memmap2::Mmap;

pub struct DenseIdToTitleMmap {
    pub titles: Mmap,  // Vec<u8> character blob
    pub offsets: Mmap, // Vec<u32>
}

impl DenseIdToTitleMmap {
    pub fn get(&self, dense_id: u32) -> &str {
        let offsets: &[u32] = util::mmap_as_u32_slice(&self.offsets);
        let start = offsets[dense_id as usize] as usize;
        let end = offsets[dense_id as usize + 1] as usize;
        std::str::from_utf8(&self.titles[start..end]).unwrap()
    }
}

pub fn build_and_save_dense_id_to_title_mmap() -> anyhow::Result<()> {
    let dense_id_to_title: Vec<String> = util::load_from_file("data/dense_id_to_title.bin")?;

    // Prepare the flat titles buffer and offsets
    let mut titles: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(dense_id_to_title.len() + 1);
    offsets.push(0);

    for title in &dense_id_to_title {
        titles.extend_from_slice(title.as_bytes());
        offsets.push(titles.len() as u32);
    }

    // Save to disk
    util::write_u8_vec_to_file(&titles, "data/dense_id_to_title/titles.bin")?;
    util::write_u32_vec_to_file(&offsets, "data/dense_id_to_title/offsets.bin")?;

    Ok(())
}

pub fn load_dense_id_to_title_mmap() -> anyhow::Result<DenseIdToTitleMmap> {
    let titles: Mmap = util::mmap_file("data/dense_id_to_title/titles.bin")?;
    let offsets: Mmap = util::mmap_file("data/dense_id_to_title/offsets.bin")?;

    Ok(DenseIdToTitleMmap { titles, offsets })
}
