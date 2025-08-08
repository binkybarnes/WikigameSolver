use rustc_hash::FxHashMap;
use std::collections::VecDeque;

pub fn bfs_adj_list(
    graph: &FxHashMap<u32, Vec<u32>>,
    redirect_targets: &FxHashMap<u32, u32>,
    start: u32,
    goal: u32,
    max_depth: u8,
) -> Option<Vec<u32>> {
    let mut queue = VecDeque::new();
    let mut parents = FxHashMap::default();
    // if a node is a redirect, resolve it but store it as a value (redirect target: redirect)
    let mut redirects_passed = FxHashMap::default();

    parents.insert(start, start); // mark start as visited
    queue.push_back(start);

    let mut depth = 0;

    while !queue.is_empty() {
        let level_size = queue.len();

        println!("Depth {}", depth);
        if depth >= max_depth {
            println!("MAX DEPTH REACHED");
            return None;
        }

        for _ in 0..level_size {
            let mut node = queue.pop_front().unwrap();
            if let Some(&redirect_target) = redirect_targets.get(&node) {
                redirects_passed.insert(redirect_target, node);
                node = redirect_target;
            }

            if node == goal {
                // reconstruct path
                let mut path = vec![node];
                let mut current = node;
                while let Some(&parent_raw) = parents.get(&current) {
                    let mut parent = parent_raw;
                    if let Some(&redirect) = redirects_passed.get(&parent) {
                        parent = redirect;
                    }
                    if parent == current {
                        break;
                    }
                    path.push(parent);
                    current = parent;
                }
                path.reverse();
                return Some(path);
            }

            if let Some(neighbors) = graph.get(&node) {
                for &neighbor in neighbors {
                    if !parents.contains_key(&neighbor) {
                        parents.insert(neighbor, node);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        depth += 1;
    }

    None
}
