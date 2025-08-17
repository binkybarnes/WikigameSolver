use crate::util;
use memmap2::Mmap;
use rustc_hash::FxHashMap;
use std::cmp::Ordering;

// going to replace pub redirects_passed: FxHashMap<(u32, u32), u32>, inside of csr_graph
pub struct RedirectsPassedMmap {
    pub offsets: Mmap,               // Vec<u32>
    pub redirect_targets_mmap: Mmap, // Vec<u32>
    pub redirects_mmap: Mmap,        // Vec<u32>
                                     // dense ids
}

impl RedirectsPassedMmap {
    /// Return the redirect for a given (page_from, target_id) if it exists
    pub fn get(&self, page_from: u32, target_id: u32) -> Option<u32> {
        let offsets: &[u32] = util::mmap_as_u32_slice(&self.offsets);

        let start = offsets[page_from as usize] as usize;
        let end = offsets[page_from as usize + 1] as usize;

        let targets_bytes: &[u8] = &self.redirect_targets_mmap;
        let redirects_bytes: &[u8] = &self.redirects_mmap;

        // Interpret bytes as u32 array (little endian assumed)
        let targets: &[u32] = bytemuck::cast_slice(&targets_bytes[start * 4..end * 4]);
        let redirects: &[u32] = bytemuck::cast_slice(&redirects_bytes[start * 4..end * 4]);

        // Binary search within this page's subsection
        let mut lo = 0usize;
        let mut hi = targets.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            match targets[mid].cmp(&target_id) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(redirects[mid]),
            }
        }
        None
    }
}

pub fn build_and_save_redirects_passed_mmap() -> anyhow::Result<()> {
    // Load original data
    let redirects_passed_dense: FxHashMap<(u32, u32), u32> =
        util::load_from_file("data/redirects_passed_dense.bin")?;
    let dense_id_to_orig: Vec<u32> = util::load_from_file("data/dense_id_to_orig.bin")?;
    let num_pages = dense_id_to_orig.len();

    // Step 1: group by page_from
    let mut grouped: FxHashMap<u32, Vec<(u32, u32)>> = FxHashMap::default();
    for (&(page_from, target), &redir) in redirects_passed_dense.iter() {
        grouped.entry(page_from).or_default().push((target, redir));
    }

    // Step 2: sort each page_from's redirects by target
    for vec in grouped.values_mut() {
        vec.sort_by_key(|&(target, _)| target);
    }

    // Step 3: build offsets, redirect_targets, and redirects
    let mut offsets = Vec::with_capacity(num_pages + 1);
    let mut redirect_targets: Vec<u32> = Vec::new();
    let mut redirects: Vec<u32> = Vec::new();
    offsets.push(0);

    for page_id in 0..num_pages {
        if let Some(pairs) = grouped.get(&(page_id as u32)) {
            for &(target, redir) in pairs {
                redirect_targets.push(target);
                redirects.push(redir);
            }
        }
        offsets.push(redirect_targets.len() as u32);
    }

    // Step 4: save to disk
    util::write_u32_vec_to_file(&offsets, "data/redirects_passed/offsets.bin")?;
    util::write_u32_vec_to_file(
        &redirect_targets,
        "data/redirects_passed/redirect_targets.bin",
    )?;
    util::write_u32_vec_to_file(&redirects, "data/redirects_passed/redirects.bin")?;

    Ok(())
}

pub fn load_redirects_passed_mmap() -> anyhow::Result<RedirectsPassedMmap> {
    let offsets = util::mmap_file("data/redirects_passed/offsets.bin")?;
    let redirect_targets_mmap = util::mmap_file("data/redirects_passed/redirect_targets.bin")?;
    let redirects_mmap = util::mmap_file("data/redirects_passed/redirects.bin")?;

    Ok(RedirectsPassedMmap {
        offsets,
        redirect_targets_mmap,
        redirects_mmap,
    })
}

// pub trait RedirectsPassedTrait {
//     fn get_redirect(&self, from: u32, to: u32) -> Option<u32>;
// }
// impl RedirectsPassedTrait for FxHashMap<(u32, u32), u32> {
//     fn get_redirect(&self, from: u32, to: u32) -> Option<u32> {
//         self.get(&(from, to)).copied()
//     }
// }

// impl RedirectsPassedTrait for RedirectsPassedMmap {
//     fn get_redirect(&self, from: u32, to: u32) -> Option<u32> {
//         self.get(from, to)
//     }
// }
