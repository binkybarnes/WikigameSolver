use rustc_hash::FxHashMap;
use std::{collections::VecDeque, time::Instant};

pub fn bfs_adj_list(
    graph: &FxHashMap<u32, Vec<u32>>,
    redirect_targets: &FxHashMap<u32, u32>,
    start: u32,
    goal: u32,
    max_depth: u8,
) -> Option<Vec<u32>> {
    let now = Instant::now();

    let mut queue = VecDeque::new();
    let mut parents = FxHashMap::default();
    // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
    // (redirect target, page): target   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
    let mut redirects_passed: FxHashMap<(u32, u32), u32> = FxHashMap::default();

    parents.insert(start, start); // mark start as visited
    queue.push_back(start);

    let mut depth = 1;

    while !queue.is_empty() {
        println!("Depth {}", depth);
        if depth >= max_depth {
            println!("MAX DEPTH REACHED");
            return None;
        }

        let level_size = queue.len();
        for _ in 0..level_size {
            let node = queue.pop_front().unwrap();

            if let Some(neighbors) = graph.get(&node) {
                for &raw_neighbor in neighbors {
                    let mut neighbor = raw_neighbor;
                    if let Some(&redirect_target) = redirect_targets.get(&neighbor) {
                        redirects_passed.insert((redirect_target, node), neighbor);
                        neighbor = redirect_target;
                    }
                    if !parents.contains_key(&neighbor) {
                        parents.insert(neighbor, node);
                        queue.push_back(neighbor);
                        if neighbor == goal {
                            let elapsed = now.elapsed();
                            println!("Elapsed: {:.2?}", elapsed);
                            return Some(reconstruct_path(
                                start,
                                goal,
                                &parents,
                                &redirects_passed,
                            ));
                        }
                    }
                }
            }
        }

        depth += 1;
    }

    None
}

pub fn reconstruct_path(
    start: u32,
    goal: u32,
    parents: &FxHashMap<u32, u32>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
) -> Vec<u32> {
    // reconstruct path
    let mut path = Vec::new();
    let mut current = goal;
    loop {
        path.push(current);
        if let Some(&parent) = parents.get(&current) {
            current = parent;
        }
        if current == start {
            break;
        }
    }
    path.push(start);
    path.reverse();

    // turn the target back into the redirect that led it there
    let mut resolved_path = Vec::new();
    resolved_path.push(start);
    for window in path.windows(2) {
        let prev_node = window[0];
        let node = window[1];
        resolved_path.push(
            redirects_passed
                .get(&(node, prev_node))
                .copied()
                .unwrap_or(node),
        );
    }
    return resolved_path;
}

// some cases for redirects
// Ziploc -> Star_Realms
// none of these should be redirects
// [1496370 ziploc,
// 3434750 united states,
// 31669618 video games in the united states, (not video gaming in the united states)
// 41506963 deck-building game, (not deck-building)
// 46365878 star realms]

// plastic bag -> plastic bag ban
// [1613879 plastic bag
// 70691392 Phase-out_of_lightweight_plastic_bags    this one is a redirect on plastic bag's page.
