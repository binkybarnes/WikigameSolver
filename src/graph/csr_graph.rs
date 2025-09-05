use bitcode::{Decode, Encode};
use memmap2::Mmap;
use rustc_hash::FxHashMap;

use crate::util;

pub trait CsrGraphTrait {
    fn get(&self, dense_node: u32) -> &[u32];
    fn get_reverse(&self, dense_node: u32) -> &[u32];
    fn num_nodes(&self) -> usize;
}

#[derive(Encode, Decode)]
pub struct CsrGraph {
    pub offsets: Vec<u32>,
    pub edges: Vec<u32>,
    pub reverse_offsets: Vec<u32>,
    pub reverse_edges: Vec<u32>,
}

impl CsrGraphTrait for CsrGraph {
    fn get(&self, dense_node: u32) -> &[u32] {
        let start = self.offsets[dense_node as usize] as usize;
        let end = self.offsets[dense_node as usize + 1] as usize;
        &self.edges[start..end]
    }
    fn get_reverse(&self, dense_node: u32) -> &[u32] {
        let start = self.reverse_offsets[dense_node as usize] as usize;
        let end = self.reverse_offsets[dense_node as usize + 1] as usize;
        &self.reverse_edges[start..end]
    }
    fn num_nodes(&self) -> usize {
        self.offsets.len() - 1
    }
}

pub struct CsrGraphMmap {
    pub offsets: Mmap,
    pub reverse_offsets: Mmap,
    pub edges_mmap: Mmap,
    pub reverse_edges_mmap: Mmap,
}

impl CsrGraphTrait for CsrGraphMmap {
    fn get(&self, dense_node: u32) -> &[u32] {
        let offsets: &[u32] = util::mmap_as_u32_slice(&self.offsets);
        let edges: &[u32] = util::mmap_as_u32_slice(&self.edges_mmap);
        let start = offsets[dense_node as usize] as usize;
        let end = offsets[dense_node as usize + 1] as usize;
        &edges[start..end]
    }
    fn get_reverse(&self, dense_node: u32) -> &[u32] {
        let reverse_offsets: &[u32] = util::mmap_as_u32_slice(&self.reverse_offsets);
        let revedges: &[u32] = util::mmap_as_u32_slice(&self.reverse_edges_mmap);
        let start = reverse_offsets[dense_node as usize] as usize;
        let end = reverse_offsets[dense_node as usize + 1] as usize;
        &revedges[start..end]
    }
    fn num_nodes(&self) -> usize {
        let offsets: &[u32] = util::mmap_as_u32_slice(&self.offsets);
        offsets.len().saturating_sub(1)
    }
}

pub fn build_csr_with_adjacency_list(
    orig_to_dense_id: &FxHashMap<u32, u32>,
    adjacency_list: &FxHashMap<u32, Vec<u32>>,
    reverse_adjacency_list: &FxHashMap<u32, Vec<u32>>,
) -> CsrGraph {
    let num_nodes = orig_to_dense_id.len();

    let mut offsets = Vec::with_capacity(num_nodes + 1);
    let mut reverse_offsets = Vec::with_capacity(num_nodes + 1);
    let mut edges = Vec::new();
    let mut reverse_edges = Vec::new();
    offsets.push(0);
    reverse_offsets.push(0);

    for dense_id in 0..num_nodes {
        let dense_id = dense_id as u32;
        if let Some(mut dense_neighbors) = adjacency_list.get(&dense_id).cloned() {
            dense_neighbors.sort_unstable();
            edges.extend(dense_neighbors);
        }
        offsets.push(edges.len() as u32);

        if let Some(mut dense_neighbors) = reverse_adjacency_list.get(&dense_id).cloned() {
            dense_neighbors.sort_unstable();
            reverse_edges.extend(dense_neighbors);
        }
        reverse_offsets.push(reverse_edges.len() as u32);
    }

    CsrGraph {
        offsets,
        edges,
        reverse_offsets,
        reverse_edges,
    }
}
