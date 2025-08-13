use rustc_hash::FxHashMap;
use std::{
    collections::VecDeque,
    io::{self, Write},
    thread::current,
    time::Instant,
};

use crate::parsers::pagelinks_parser;

fn process_neighbor(
    neighbor: u32,
    next_depth: u8,
    target: u32,
    visited_depth: &mut FxHashMap<u32, u8>,
    parents: &mut FxHashMap<u32, Vec<u32>>,
    node: u32,
    goal_found_at_depth: &mut Option<u8>,
    queue: &mut VecDeque<(u32, u8)>,
) {
    match visited_depth.get(&neighbor) {
        // Case 1: Never seen this neighbor before. This is a valid new path.
        None => {
            visited_depth.insert(neighbor, next_depth);
            parents.insert(neighbor, vec![node]);
            queue.push_back((neighbor, next_depth)); // now we can push here directly
        }
        // Case 2: Seen this neighbor before AT THE SAME DEPTH. This is a valid parallel path.
        Some(&depth) if depth == next_depth => {
            // its possible when has both page -> redirect neighbor and page -> neighbor
            // you can add a check here to prevent duplicate or remove duplicates in reconstruct
            // let parents_for_neighbor = parents.get_mut(&neighbor).unwrap();
            // if !parents_for_neighbor.contains(&node) {
            //     parents_for_neighbor.push(node);
            // }
            parents.get_mut(&neighbor).unwrap().push(node);
        }
        // Case 3: Seen this neighbor before at an earlier depth. This is a longer path or a cycle. Ignore it.
        _ => (),
    }

    if neighbor == target && goal_found_at_depth.is_none() {
        println!("path found at depth {}", next_depth);
        *goal_found_at_depth = Some(next_depth);
    }
}

// ilisendipede something -> Leptorhynchoididae (redirect to target)
pub fn bfs_adj_list(
    graph: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    start: u32,
    goal: u32,
    max_depth: u8,
) -> Option<Vec<Vec<u32>>> {
    let now = Instant::now();

    // DO NOT PASS IN REDIRECTS AS START AND GOAL

    // case where start is same as goal (can happen when the start is a redirect to the goal)
    if start == goal {
        return Some(vec![vec![start]]);
    }

    let mut queue = VecDeque::new();
    // going to make it so a node can have multiple parents (for multiple shortest paths)
    let mut parents: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    // now that a node can have multiple parents, i have to make sure the neighbors are on the same depth, or there will be a loop
    let mut visited_depth: FxHashMap<u32, u8> = FxHashMap::default();

    queue.push_back((start, 0));
    visited_depth.insert(start, 0);

    let mut goal_found_at_depth: Option<u8> = None;
    let mut prev_depth = 0;

    while let Some((node, current_depth)) = queue.pop_front() {
        if prev_depth != current_depth {
            println!("Depth {}", current_depth);
            prev_depth = current_depth;
        }
        // If we have already found the goal, finish this depth and then stop
        if let Some(depth) = goal_found_at_depth {
            if current_depth >= depth {
                break;
            }
        }

        // Check max depth
        if current_depth >= max_depth {
            println!("MAX DEPTH REACHED");
            return None;
        }
        let next_depth = current_depth + 1;

        if let Some(neighbors) = graph.get(&node) {
            for &neighbor in neighbors {
                process_neighbor(
                    neighbor,
                    next_depth,
                    goal,
                    &mut visited_depth,
                    &mut parents,
                    node,
                    &mut goal_found_at_depth,
                    &mut queue,
                );
            }
        }
    }

    if goal_found_at_depth.is_some() {
        let elapsed = now.elapsed();
        println!("Elapsed: {:.2?}", elapsed);
        return Some(reconstruct_all_paths(
            start,
            goal,
            &parents,
            &redirects_passed,
            true,
            false,
        ));
    }

    None
}

pub fn bfs_adj_list_backwards(
    graph: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    start: u32,
    goal: u32,
    max_depth: u8,
) -> Option<Vec<Vec<u32>>> {
    let now = Instant::now();

    // case where start is same as goal (can happen when the start is a redirect to the goal)
    if start == goal {
        return Some(vec![vec![goal]]);
    }

    let mut queue = VecDeque::new();
    // going to make it so a node can have multiple parents (for multiple shortest paths)
    let mut parents: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    // now that a node can have multiple parents, i have to make sure the neighbors are on the same depth, or there will be a loop
    let mut visited_depth: FxHashMap<u32, u8> = FxHashMap::default();

    queue.push_back((goal, 0));
    visited_depth.insert(goal, 0);

    let mut goal_found_at_depth: Option<u8> = None;
    let mut prev_depth = 0;

    while let Some((node, current_depth)) = queue.pop_front() {
        if prev_depth != current_depth {
            println!("Depth {}", current_depth);
            prev_depth = current_depth;
        }
        // If we have already found the goal, finish this depth and then stop
        if let Some(depth) = goal_found_at_depth {
            if current_depth >= depth {
                break;
            }
        }

        // Check max depth
        if current_depth >= max_depth {
            println!("MAX DEPTH REACHED");
            return None;
        }
        let next_depth = current_depth + 1;

        if let Some(neighbors) = graph.get(&node) {
            for &neighbor in neighbors {
                process_neighbor(
                    neighbor,
                    next_depth,
                    start,
                    &mut visited_depth,
                    &mut parents,
                    node,
                    &mut goal_found_at_depth,
                    &mut queue,
                );
            }
        }
    }

    if goal_found_at_depth.is_some() {
        let elapsed = now.elapsed();
        println!("Elapsed: {:.2?}", elapsed);
        return Some(reconstruct_all_paths(
            goal,
            start,
            &parents,
            &redirects_passed,
            true,
            true,
        ));
    }

    None
}

pub fn bfs_adj_list_backwards1(
    graph: &FxHashMap<u32, Vec<u32>>,
    redirect_targets: &FxHashMap<u32, u32>,
    orig_start: u32,
    orig_goal: u32,
    max_depth: u8,
) -> Option<Vec<Vec<u32>>> {
    let now = Instant::now();

    let start = redirect_targets
        .get(&orig_start)
        .copied()
        .unwrap_or(orig_start);
    let goal = redirect_targets
        .get(&orig_goal)
        .copied()
        .unwrap_or(orig_goal);

    // case where start is same as goal (can happen when the start is a redirect to the goal)
    if start == goal {
        return Some(vec![vec![orig_goal]]);
    }

    let mut queue = VecDeque::new();
    let mut parents: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
    // (page, redirect target): target   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
    let mut redirects_passed: FxHashMap<(u32, u32), u32> = FxHashMap::default();

    parents.insert(goal, vec![goal]); // mark start as visited
    queue.push_back(goal);

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
                    // if the neighbor is a redirect, add its neighbors to the queue instead
                    if redirect_targets.contains_key(&raw_neighbor) {
                        if let Some(redirect_neighbors) = graph.get(&raw_neighbor) {
                            for &redirect_neighbor in redirect_neighbors {
                                redirects_passed.insert((redirect_neighbor, node), raw_neighbor);

                                if !parents.contains_key(&redirect_neighbor) {
                                    // parents.insert(redirect_neighbor, node);
                                    parents.entry(redirect_neighbor).or_default().push(node);
                                    if redirect_neighbor == start {
                                        let elapsed = now.elapsed();
                                        println!("Elapsed: {:.2?}", elapsed);
                                        return Some(reconstruct_all_paths(
                                            goal,
                                            start,
                                            &parents,
                                            &redirects_passed,
                                            true,
                                            true,
                                        ));
                                    }
                                    queue.push_back(redirect_neighbor);
                                }
                            }
                        }
                    } else {
                        if !parents.contains_key(&raw_neighbor) {
                            // parents.insert(raw_neighbor, node);
                            parents.entry(raw_neighbor).or_default().push(node);

                            if raw_neighbor == start {
                                let elapsed = now.elapsed();
                                println!("Elapsed: {:.2?}", elapsed);
                                return Some(reconstruct_all_paths(
                                    goal,
                                    start,
                                    &parents,
                                    &redirects_passed,
                                    true,
                                    true,
                                ));
                            }
                            queue.push_back(raw_neighbor);
                        }
                    }
                }
            }
        }

        depth += 1;
    }

    None
}

// pub fn bi_bfs_adj_list(
//     graph_fwd: &FxHashMap<u32, Vec<u32>>,
//     graph_bwd: &FxHashMap<u32, Vec<u32>>,
//     redirect_targets: &FxHashMap<u32, u32>,
//     orig_start: u32,
//     orig_goal: u32,
//     max_depth: u8,
// ) -> Option<Vec<u32>> {
//     let now = Instant::now();

//     let start = redirect_targets
//         .get(&orig_start)
//         .copied()
//         .unwrap_or(orig_start);
//     let goal = redirect_targets
//         .get(&orig_goal)
//         .copied()
//         .unwrap_or(orig_goal);

//     // case where start is same as goal (can happen when the start is a redirect to the goal)
//     if start == goal {
//         return Some(vec![orig_start]);
//     }

//     let mut queue_fwd = VecDeque::new();
//     let mut queue_bwd = VecDeque::new();
//     let mut parents_fwd: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
//     let mut parents_bwd: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
//     // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
//     // (page, redirect target): redirect   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
//     let mut redirects_passed_fwd: FxHashMap<(u32, u32), u32> = FxHashMap::default();
//     let mut redirects_passed_bwd: FxHashMap<(u32, u32), u32> = FxHashMap::default();

//     parents_fwd.insert(start, vec![start]); // mark start as visited
//     parents_bwd.insert(goal, vec![goal]); // mark goal as visited
//     queue_fwd.push_back(start);
//     queue_bwd.push_back(goal);

//     let mut depth_fwd = 0;
//     let mut depth_bwd = 0;

//     while !queue_fwd.is_empty() && !queue_bwd.is_empty() {
//         let (queue, parents_this, parents_other, redirects_passed, graph, fwd_smaller) =
//             if queue_fwd.len() <= queue_bwd.len() {
//                 (
//                     &mut queue_fwd,
//                     &mut parents_fwd,
//                     &mut parents_bwd,
//                     &mut redirects_passed_fwd,
//                     graph_fwd,
//                     true,
//                 )
//             } else {
//                 (
//                     &mut queue_bwd,
//                     &mut parents_bwd,
//                     &mut parents_fwd,
//                     &mut redirects_passed_bwd,
//                     graph_bwd,
//                     false,
//                 )
//             };

//         if fwd_smaller {
//             depth_fwd += 1;
//             println!("Depth forward {}", depth_fwd);
//             if depth_fwd + depth_bwd >= max_depth {
//                 println!("MAX DEPTH REACHED");
//                 return None;
//             }
//         } else {
//             depth_bwd += 1;
//             println!("Depth backwards {}", depth_bwd);
//             if depth_fwd + depth_bwd >= max_depth {
//                 println!("MAX DEPTH REACHED");
//                 return None;
//             }
//         }

//         let level_size = queue.len();
//         for _ in 0..level_size {
//             let node = queue.pop_front().unwrap();

//             if let Some(neighbors) = graph.get(&node) {
//                 for &raw_neighbor in neighbors {
//                     if fwd_smaller {
//                         // FORWARD
//                         let neighbor =
//                             if let Some(&redirect_target) = redirect_targets.get(&raw_neighbor) {
//                                 redirects_passed.insert((node, redirect_target), raw_neighbor);
//                                 redirect_target
//                             } else {
//                                 raw_neighbor
//                             };
//                         if !parents_this.contains_key(&neighbor) {
//                             // parents_this.insert(neighbor, node);
//                             parents_this.entry(neighbor).or_default().push(node);

//                             // check for meeting point
//                             if parents_other.contains_key(&neighbor) {
//                                 println!("Elapsed: {:.2?}", now.elapsed());
//                                 return Some(merge_paths(
//                                     start,
//                                     goal,
//                                     neighbor,
//                                     &parents_fwd,
//                                     &parents_bwd,
//                                     &redirects_passed_fwd,
//                                     &redirects_passed_bwd,
//                                 ));
//                             }
//                             queue.push_back(neighbor);
//                         }
//                     } else {
//                         // BACKWARDS
//                         // if the neighbor is a redirect, add its neighbors to the queue instead
//                         if redirect_targets.contains_key(&raw_neighbor) {
//                             if let Some(redirect_neighbors) = graph.get(&raw_neighbor) {
//                                 for &redirect_neighbor in redirect_neighbors {
//                                     redirects_passed
//                                         .insert((redirect_neighbor, node), raw_neighbor);

//                                     if !parents_this.contains_key(&redirect_neighbor) {
//                                         parents_this.insert(redirect_neighbor, node);
//                                         // check for meeting point
//                                         if parents_other.contains_key(&redirect_neighbor) {
//                                             println!("Elapsed: {:.2?}", now.elapsed());
//                                             return Some(merge_paths(
//                                                 start,
//                                                 goal,
//                                                 redirect_neighbor,
//                                                 &parents_fwd,
//                                                 &parents_bwd,
//                                                 &redirects_passed_fwd,
//                                                 &redirects_passed_bwd,
//                                             ));
//                                         }
//                                         queue.push_back(redirect_neighbor);
//                                     }
//                                 }
//                             }
//                         } else {
//                             if !parents_this.contains_key(&raw_neighbor) {
//                                 parents_this.insert(raw_neighbor, node);
//                                 // check for meeting point
//                                 if parents_other.contains_key(&raw_neighbor) {
//                                     println!("Elapsed: {:.2?}", now.elapsed());
//                                     return Some(merge_paths(
//                                         start,
//                                         goal,
//                                         raw_neighbor,
//                                         &parents_fwd,
//                                         &parents_bwd,
//                                         &redirects_passed_fwd,
//                                         &redirects_passed_bwd,
//                                     ));
//                                 }
//                                 queue.push_back(raw_neighbor);
//                             }
//                         }
//                     }
//                 }
//             }
//         }
//     }

//     None
// }

pub fn reconstruct_all_paths(
    start: u32,
    goal: u32,
    parents: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    return_redirects: bool,
    reverse: bool,
) -> Vec<Vec<u32>> {
    let mut all_paths = Vec::new();

    // Stack holds (current_node, current_path)
    // current_path is goal->...->current_node order
    let mut stack: Vec<(u32, Vec<u32>)> = Vec::new();
    stack.push((goal, vec![goal]));

    while let Some((node, path)) = stack.pop() {
        if node == start {
            let mut complete_path = path.clone();
            complete_path.reverse(); // make it start->...->goal
            all_paths.push(complete_path);
        } else if let Some(pars) = parents.get(&node) {
            for &p in pars {
                let mut new_path = path.clone();
                new_path.push(p);
                stack.push((p, new_path));
            }
        }
    }

    // Deduplicate paths using a HashSet
    let mut seen = HashSet::new();
    let mut unique_paths = Vec::with_capacity(all_paths.len());
    for path in all_paths {
        if seen.insert(path.clone()) {
            unique_paths.push(path);
        }
    }

    // If reverse flag is set, reverse each found path back
    if reverse {
        for path in &mut unique_paths {
            path.reverse();
        }
    }

    if !return_redirects {
        return unique_paths;
    }

    // Apply redirect resolution
    let mut resolved_paths = Vec::new();
    for path in unique_paths {
        if path.len() < 2 {
            resolved_paths.push(path);
            continue;
        }

        let mut resolved_path = Vec::with_capacity(path.len());
        resolved_path.push(path[0]);
        for window in path.windows(2) {
            let prev_node = window[0];
            let node = window[1];
            resolved_path.push(*redirects_passed.get(&(prev_node, node)).unwrap_or(&node));
        }
        resolved_paths.push(resolved_path);
    }

    resolved_paths
}

// pub fn reconstruct_path(
//     start: u32,
//     goal: u32,
//     parents: &FxHashMap<u32, u32>,
//     redirects_passed: &FxHashMap<(u32, u32), u32>,
//     return_redirects: bool,
//     reverse: bool,
// ) -> Vec<u32> {
//     // reconstruct path
//     let mut path = Vec::new();
//     let mut current = goal;
//     loop {
//         path.push(current);
//         if current == start {
//             break;
//         }
//         let &parent = parents.get(&current).unwrap();
//         current = parent;
//     }

//     // this looks funny but the forward path should be reversed since you start parent backtracking from the goal, backwards should not since it already starts at the goal
//     if !reverse {
//         path.reverse()
//     }

//     if !return_redirects {
//         return path;
//     };

//     // turn the target back into the redirect that led it there
//     let mut resolved_path = Vec::new();
//     resolved_path.push(path[0]);
//     for window in path.windows(2) {
//         let prev_node = window[0];
//         let node = window[1];
//         resolved_path.push(
//             redirects_passed
//                 .get(&(prev_node, node))
//                 .copied()
//                 .unwrap_or(node),
//         );
//     }
//     return resolved_path;
// }

// // when using incoming links
// pub fn reconstruct_path_backwards(
//     start: u32,
//     goal: u32,
//     parents: &FxHashMap<u32, u32>,
//     redirects_passed: &FxHashMap<(u32, u32), u32>,
//     return_redirects: bool,
// ) -> Vec<u32> {
//     // reconstruct path
//     let mut path = Vec::new();
//     let mut current = start;
//     loop {
//         path.push(current);
//         if current == goal {
//             break;
//         }
//         let &parent = parents.get(&current).unwrap();
//         current = parent;
//     }
//     if !return_redirects {
//         return path;
//     };

//     // turn the target back into the redirect that led it there
//     let mut resolved_path = Vec::new();
//     resolved_path.push(path[0]);
//     for window in path.windows(2) {
//         let prev_node = window[0];
//         let node = window[1];
//         resolved_path.push(
//             redirects_passed
//                 .get(&(prev_node, node))
//                 .copied()
//                 .unwrap_or(node),
//         );
//     }
//     return resolved_path;
// }

// fn merge_paths(
//     start: u32,
//     goal: u32,
//     meet: u32,
//     parents_fwd: &FxHashMap<u32, Vec<u32>>,
//     parents_bwd: &FxHashMap<u32, Vec<u32>>,
//     redirects_fwd: &FxHashMap<(u32, u32), u32>,
//     redirects_bwd: &FxHashMap<(u32, u32), u32>,
// ) -> Vec<u32> {
//     let mut path_fwd = reconstruct_all_paths(start, meet, parents_fwd, redirects_fwd, true, false);
//     let path_bwd = reconstruct_all_paths(goal, meet, parents_bwd, redirects_bwd, true, true);

//     // do not pop from path_fwd.pop(); if the meet point is supposed to be a redirect, it only gets put back in path_fwd
//     // because for path_bwd the meet point is at the front which doesn't get changes
//     // ex: Forward path: [1613879 Plastic_bag, 70691392 Phase-out_of_lightweight_plastic_bags] Backward path: [36080727 Plastic_bag_ban]

//     // remove duplicate meet point (first element of path_bwd)
//     path_fwd.extend(&path_bwd[1..]);
//     path_fwd
// }

// fn bfs_csr(
//     graph: &pagelinks_parser::CsrGraph,
//     orig_start: u32,
//     orig_goal: u32,
//     max_depth: u8,
// ) -> Option<Vec<u32>> {
//     let now = Instant::now();

//     let mut start = graph.orig_to_dense.get(&orig_start).copied()?;
//     let mut goal = graph.orig_to_dense.get(&orig_goal).copied()?;

//     // if start or goal is redirect, resolve it
//     start = graph.resolve_redirect(start).unwrap_or(start);
//     goal = graph.resolve_redirect(goal).unwrap_or(goal);

//     // case where start is same as goal (can happen when the start is a redirect to the goal)
//     if start == goal {
//         return Some(vec![orig_start]);
//     }

//     let mut queue = VecDeque::new();
//     let mut parents: FxHashMap<u32, u32> = FxHashMap::default();
//     // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
//     // (page, redirect target): redirect   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
//     let mut redirects_passed: FxHashMap<(u32, u32), u32> = FxHashMap::default();

//     parents.insert(start, start); // mark start as visited
//     queue.push_back(start);

//     let mut depth = 1;

//     while !queue.is_empty() {
//         println!("Depth {}", depth);
//         if depth >= max_depth {
//             println!("MAX DEPTH REACHED");
//             return None;
//         }

//         let level_size = queue.len();
//         for _ in 0..level_size {
//             let node = queue.pop_front().unwrap();

//             let neighbors = graph.get(node);
//             for &raw_neighbor in neighbors {
//                 let neighbor = if let Some(redirect_target) = graph.resolve_redirect(raw_neighbor) {
//                     redirects_passed.insert((node, redirect_target), raw_neighbor);
//                     redirect_target
//                 } else {
//                     raw_neighbor
//                 };

//                 if !parents.contains_key(&neighbor) {
//                     parents.insert(neighbor, node);
//                     if neighbor == goal {
//                         let elapsed = now.elapsed();
//                         println!("Elapsed: {:.2?}", elapsed);
//                         return Some(reconstruct_path_csr(
//                             start,
//                             goal,
//                             &parents,
//                             &redirects_passed,
//                             &graph.dense_to_orig,
//                             true,
//                             false,
//                         ));
//                     }
//                     queue.push_back(neighbor);
//                 }
//             }
//         }

//         depth += 1;
//     }

//     None
// }

// fn bfs_csr_backwards(
//     graph: &pagelinks_parser::CsrGraph,
//     orig_start: u32,
//     orig_goal: u32,
//     max_depth: u8,
// ) -> Option<Vec<u32>> {
//     let now = Instant::now();

//     let mut start = graph.orig_to_dense.get(&orig_start).copied()?;
//     let mut goal = graph.orig_to_dense.get(&orig_goal).copied()?;

//     // if start or goal is redirect, resolve it
//     start = graph.resolve_redirect(start).unwrap_or(start);
//     goal = graph.resolve_redirect(goal).unwrap_or(goal);

//     // case where start is same as goal (can happen when the start is a redirect to the goal)
//     if start == goal {
//         return Some(vec![orig_goal]);
//     }

//     let mut queue = VecDeque::new();
//     let mut parents: FxHashMap<u32, u32> = FxHashMap::default();
//     // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
//     // (page, redirect target): redirect   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
//     let mut redirects_passed: FxHashMap<(u32, u32), u32> = FxHashMap::default();

//     parents.insert(goal, goal); // mark start as visited
//     queue.push_back(goal);

//     let mut depth = 1;

//     while !queue.is_empty() {
//         println!("Depth {}", depth);
//         if depth >= max_depth {
//             println!("MAX DEPTH REACHED");
//             return None;
//         }

//         let level_size = queue.len();
//         for _ in 0..level_size {
//             let node = queue.pop_front().unwrap();

//             let neighbors = graph.get_reverse(node);
//             for &raw_neighbor in neighbors {
//                 // BACKWARDS
//                 // if the neighbor is a redirect, add its neighbors to the queue instead
//                 if graph.resolve_redirect(raw_neighbor).is_some() {
//                     let redirect_neighbors = graph.get_reverse(raw_neighbor);
//                     for &redirect_neighbor in redirect_neighbors {
//                         redirects_passed.insert((redirect_neighbor, node), raw_neighbor);

//                         if !parents.contains_key(&redirect_neighbor) {
//                             parents.insert(redirect_neighbor, node);
//                             if redirect_neighbor == start {
//                                 let elapsed = now.elapsed();
//                                 println!("Elapsed: {:.2?}", elapsed);
//                                 return Some(reconstruct_path_csr(
//                                     goal,
//                                     start,
//                                     &parents,
//                                     &redirects_passed,
//                                     &graph.dense_to_orig,
//                                     true,
//                                     true,
//                                 ));
//                             }
//                             queue.push_back(redirect_neighbor);
//                         }
//                     }
//                 } else {
//                     if !parents.contains_key(&raw_neighbor) {
//                         parents.insert(raw_neighbor, node);
//                         if raw_neighbor == start {
//                             let elapsed = now.elapsed();
//                             println!("Elapsed: {:.2?}", elapsed);
//                             return Some(reconstruct_path_csr(
//                                 goal,
//                                 start,
//                                 &parents,
//                                 &redirects_passed,
//                                 &graph.dense_to_orig,
//                                 true,
//                                 true,
//                             ));
//                         }
//                         queue.push_back(raw_neighbor);
//                     }
//                 }
//             }
//         }

//         depth += 1;
//     }

//     None
// }

// pub fn bi_bfs_csr(
//     graph: &pagelinks_parser::CsrGraph,
//     orig_start: u32,
//     orig_goal: u32,
//     max_depth: u8,
// ) -> Option<Vec<u32>> {
//     let now = Instant::now();

//     let mut start = graph.orig_to_dense.get(&orig_start).copied()?;
//     let mut goal = graph.orig_to_dense.get(&orig_goal).copied()?;

//     // if start or goal is redirect, resolve it
//     start = graph.resolve_redirect(start).unwrap_or(start);
//     goal = graph.resolve_redirect(goal).unwrap_or(goal);

//     // case where start is same as goal (can happen when the start is a redirect to the goal)
//     if start == goal {
//         return Some(vec![orig_start]);
//     }

//     let mut queue_fwd = VecDeque::new();
//     let mut queue_bwd = VecDeque::new();
//     let mut parents_fwd = FxHashMap::default();
//     let mut parents_bwd = FxHashMap::default();
//     // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
//     // (page, redirect target): redirect   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
//     let mut redirects_passed_fwd: FxHashMap<(u32, u32), u32> = FxHashMap::default();
//     let mut redirects_passed_bwd: FxHashMap<(u32, u32), u32> = FxHashMap::default();

//     parents_fwd.insert(start, start); // mark start as visited
//     parents_bwd.insert(goal, goal); // mark goal as visited
//     queue_fwd.push_back(start);
//     queue_bwd.push_back(goal);

//     let mut depth_fwd = 0;
//     let mut depth_bwd = 0;

//     while !queue_fwd.is_empty() && !queue_bwd.is_empty() {
//         let (queue, parents_this, parents_other, redirects_passed, fwd_smaller) =
//             if queue_fwd.len() <= queue_bwd.len() {
//                 (
//                     &mut queue_fwd,
//                     &mut parents_fwd,
//                     &mut parents_bwd,
//                     &mut redirects_passed_fwd,
//                     true,
//                 )
//             } else {
//                 (
//                     &mut queue_bwd,
//                     &mut parents_bwd,
//                     &mut parents_fwd,
//                     &mut redirects_passed_bwd,
//                     false,
//                 )
//             };
//         if fwd_smaller {
//             depth_fwd += 1;
//             println!("Depth forward {}", depth_fwd);
//             if depth_fwd + depth_bwd >= max_depth {
//                 println!("MAX DEPTH REACHED");
//                 return None;
//             }
//         } else {
//             depth_bwd += 1;
//             println!("Depth backwards {}", depth_bwd);
//             if depth_fwd + depth_bwd >= max_depth {
//                 println!("MAX DEPTH REACHED");
//                 return None;
//             }
//         }

//         let level_size = queue.len();
//         for _ in 0..level_size {
//             let node = queue.pop_front().unwrap();

//             let neighbors = if fwd_smaller {
//                 graph.get(node)
//             } else {
//                 graph.get_reverse(node)
//             };
//             for &raw_neighbor in neighbors {
//                 if fwd_smaller {
//                     // FORWARD
//                     let neighbor =
//                         if let Some(redirect_target) = graph.resolve_redirect(raw_neighbor) {
//                             redirects_passed.insert((node, redirect_target), raw_neighbor);
//                             redirect_target
//                         } else {
//                             raw_neighbor
//                         };
//                     if !parents_this.contains_key(&neighbor) {
//                         parents_this.insert(neighbor, node);
//                         // check for meeting point
//                         if parents_other.contains_key(&neighbor) {
//                             println!("Elapsed: {:.2?}", now.elapsed());
//                             return Some(merge_paths_csr(
//                                 start,
//                                 goal,
//                                 neighbor,
//                                 &parents_fwd,
//                                 &parents_bwd,
//                                 &redirects_passed_fwd,
//                                 &redirects_passed_bwd,
//                                 &graph.dense_to_orig,
//                             ));
//                         }
//                         queue.push_back(neighbor);
//                     }
//                 } else {
//                     // BACKWARDS
//                     // if the neighbor is a redirect, add its neighbors to the queue instead
//                     if graph.resolve_redirect(raw_neighbor).is_some() {
//                         let redirect_neighbors = graph.get_reverse(raw_neighbor);
//                         for &redirect_neighbor in redirect_neighbors {
//                             redirects_passed.insert((redirect_neighbor, node), raw_neighbor);

//                             if !parents_this.contains_key(&redirect_neighbor) {
//                                 parents_this.insert(redirect_neighbor, node);
//                                 // check for meeting point
//                                 if parents_other.contains_key(&redirect_neighbor) {
//                                     println!("Elapsed: {:.2?}", now.elapsed());
//                                     return Some(merge_paths_csr(
//                                         start,
//                                         goal,
//                                         redirect_neighbor,
//                                         &parents_fwd,
//                                         &parents_bwd,
//                                         &redirects_passed_fwd,
//                                         &redirects_passed_bwd,
//                                         &graph.dense_to_orig,
//                                     ));
//                                 }
//                                 queue.push_back(redirect_neighbor);
//                             }
//                         }
//                     } else {
//                         if !parents_this.contains_key(&raw_neighbor) {
//                             parents_this.insert(raw_neighbor, node);
//                             // check for meeting point
//                             if parents_other.contains_key(&raw_neighbor) {
//                                 println!("Elapsed: {:.2?}", now.elapsed());
//                                 return Some(merge_paths_csr(
//                                     start,
//                                     goal,
//                                     raw_neighbor,
//                                     &parents_fwd,
//                                     &parents_bwd,
//                                     &redirects_passed_fwd,
//                                     &redirects_passed_bwd,
//                                     &graph.dense_to_orig,
//                                 ));
//                             }
//                             queue.push_back(raw_neighbor);
//                         }
//                     }
//                 }
//             }
//         }
//     }

//     None
// }

// pub fn reconstruct_path_csr(
//     start: u32,
//     goal: u32,
//     parents: &FxHashMap<u32, u32>,
//     redirects_passed: &FxHashMap<(u32, u32), u32>,
//     dense_to_orig: &Vec<u32>,
//     return_redirects: bool,
//     reverse: bool,
// ) -> Vec<u32> {
//     let path = reconstruct_path(
//         start,
//         goal,
//         parents,
//         redirects_passed,
//         return_redirects,
//         reverse,
//     );

//     let orig_path: Vec<u32> = path
//         .into_iter()
//         .map(|node| dense_to_orig[node as usize])
//         .collect();

//     return orig_path;
// }

// pub fn reconstruct_path_csr_backwards(
//     start: u32,
//     goal: u32,
//     parents: &FxHashMap<u32, u32>,
//     redirects_passed: &FxHashMap<(u32, u32), u32>,
//     dense_to_orig: &Vec<u32>,
//     return_redirects: bool,
// ) -> Vec<u32> {
//     let path = reconstruct_path_backwards(start, goal, parents, redirects_passed, return_redirects);

//     let orig_path: Vec<u32> = path
//         .into_iter()
//         .map(|node| dense_to_orig[node as usize])
//         .collect();

//     return orig_path;
// }

// fn merge_paths_csr(
//     start: u32,
//     goal: u32,
//     meet: u32,
//     parents_fwd: &FxHashMap<u32, u32>,
//     parents_bwd: &FxHashMap<u32, u32>,
//     redirects_fwd: &FxHashMap<(u32, u32), u32>,
//     redirects_bwd: &FxHashMap<(u32, u32), u32>,
//     dense_to_orig: &Vec<u32>,
// ) -> Vec<u32> {
//     let path = merge_paths(
//         start,
//         goal,
//         meet,
//         parents_fwd,
//         parents_bwd,
//         redirects_fwd,
//         redirects_bwd,
//     );

//     let orig_path: Vec<u32> = path
//         .into_iter()
//         .map(|node| dense_to_orig[node as usize])
//         .collect();

//     return orig_path;
// }

use std::collections::HashSet;

fn paths_to_strings(
    paths: &Vec<Vec<u32>>,
    id_to_title: &FxHashMap<u32, String>,
) -> HashSet<String> {
    paths
        .iter()
        .map(|path| {
            path.iter()
                .map(|id| {
                    id_to_title
                        .get(id)
                        .map(String::as_str)
                        .unwrap_or("[Unknown]")
                })
                .collect::<Vec<_>>()
                .join(" -> ")
        })
        .collect()
}

fn print_path_examples(paths: &HashSet<String>, label: &str, max_print: usize) {
    println!("{} paths ({} examples):", label, paths.len());
    for (i, path) in paths.iter().take(max_print).enumerate() {
        println!("  {}: {}", i + 1, path);
    }
    if paths.len() > max_print {
        println!("  ...and {} more", paths.len() - max_print);
    }
}

pub fn bfs_interactive_session(
    title_to_id: &FxHashMap<String, u32>,
    id_to_title: &FxHashMap<u32, String>,
    csr_graph: &pagelinks_parser::CsrGraph,
    adj_graph: &FxHashMap<u32, Vec<u32>>,
    adj_graph_bwd: &FxHashMap<u32, Vec<u32>>,
    redirect_targets: &FxHashMap<u32, u32>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
) {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("Enter start page title (or 'exit'): ");
        stdout.flush().unwrap();
        let mut start_title = String::new();
        if stdin.read_line(&mut start_title).is_err() {
            break;
        }
        let start_title = start_title.trim();
        if start_title.eq_ignore_ascii_case("exit") {
            break;
        }

        print!("Enter goal page title (or 'exit'): ");
        stdout.flush().unwrap();
        let mut goal_title = String::new();
        if stdin.read_line(&mut goal_title).is_err() {
            break;
        }
        let goal_title = goal_title.trim();
        if goal_title.eq_ignore_ascii_case("exit") {
            break;
        }

        let Some(&start_id_raw) = title_to_id.get(start_title) else {
            println!("Start title '{}' not found in mapping.", start_title);
            continue;
        };
        let Some(&goal_id_raw) = title_to_id.get(goal_title) else {
            println!("Goal title '{}' not found in mapping.", goal_title);
            continue;
        };

        let mut start_id = start_id_raw;
        let mut goal_id = goal_id_raw;

        // Resolve redirects
        if let Some(&redirect_target) = redirect_targets.get(&start_id) {
            start_id = redirect_target;
        }
        if let Some(&redirect_target) = redirect_targets.get(&goal_id) {
            goal_id = redirect_target;
        }

        let max_depth = 50;

        println!("\nRunning forwards BFS on adjacency list graph...");
        let now = Instant::now();
        let paths_adj_fwd = bfs_adj_list(adj_graph, redirects_passed, start_id, goal_id, max_depth);
        let elapsed_fwd = now.elapsed();

        match &paths_adj_fwd {
            Some(paths) if !paths.is_empty() => {
                println!(
                    "Paths found (adjacency list) [{} shortest paths, {:.2?}]:",
                    paths.len(),
                    elapsed_fwd
                );
                let path = &paths[0];
                println!("Path 1 ({} nodes):", path.len());
                for id in path {
                    let title = id_to_title
                        .get(id)
                        .map(String::as_str)
                        .unwrap_or("[Unknown]");
                    print!("{} -> ", title);
                }
                println!("END");
            }
            _ => println!(
                "No path found in adjacency list BFS (after {:.2?}).",
                elapsed_fwd
            ),
        }

        println!("\nRunning backwards BFS on adjacency list graph...");
        let now = Instant::now();
        let paths_adj_bwd = bfs_adj_list_backwards(
            adj_graph_bwd,
            redirects_passed,
            start_id,
            goal_id,
            max_depth,
        );
        let elapsed_bwd = now.elapsed();

        match &paths_adj_bwd {
            Some(paths) if !paths.is_empty() => {
                println!(
                    "Paths found (adjacency list) [{} shortest paths, {:.2?}]:",
                    paths.len(),
                    elapsed_bwd
                );
                let path = &paths[0];
                println!("Path 1 ({} nodes):", path.len());
                for id in path {
                    let title = id_to_title
                        .get(id)
                        .map(String::as_str)
                        .unwrap_or("[Unknown]");
                    print!("{} -> ", title);
                }
                println!("END");
            }
            _ => println!(
                "No path found in adjacency list BFS (after {:.2?}).",
                elapsed_bwd
            ),
        }
        if let (Some(fwd), Some(bwd)) = (paths_adj_fwd.clone(), paths_adj_bwd.clone()) {
            let fwd_set = paths_to_strings(&fwd, id_to_title);
            let bwd_set = paths_to_strings(&bwd, id_to_title);

            println!("\nForward BFS found {} shortest paths", fwd_set.len());
            println!("Backward BFS found {} shortest paths", bwd_set.len());

            let only_in_fwd: HashSet<_> = fwd_set.difference(&bwd_set).collect();
            let only_in_bwd: HashSet<_> = bwd_set.difference(&fwd_set).collect();

            println!("Paths only in forward BFS: {}", only_in_fwd.len());
            print_path_examples(
                &only_in_fwd.iter().cloned().cloned().collect(),
                "Only in Forward",
                10,
            );

            println!("Paths only in backward BFS: {}", only_in_bwd.len());
            print_path_examples(
                &only_in_bwd.iter().cloned().cloned().collect(),
                "Only in Backward",
                10,
            );
        } else {
            println!("One or both BFS searches did not find paths, skipping comparison.");
        }

        // // Bidirectional BFS
        // println!("\nRunning Bidirectional BFS on adjacency list graph...");
        // let now = Instant::now();
        // let path_bi_adj = bi_bfs_adj_list(
        //     adj_graph,
        //     adj_graph_bwd,
        //     redirect_targets,
        //     start_id,
        //     goal_id,
        //     max_depth,
        // );
        // let elapsed_bi_adj = now.elapsed();
        // match path_bi_adj {
        //     Some(path) => {
        //         println!(
        //             "Path found (Bidirectional adjacency list) [{} nodes, {:.2?}]:",
        //             path.len(),
        //             elapsed_bi_adj
        //         );
        //         for id in path {
        //             let title = id_to_title
        //                 .get(&id)
        //                 .map(String::as_str)
        //                 .unwrap_or("[Unknown]");
        //             print!("{} -> ", title);
        //         }
        //         println!("END");
        //     }
        //     None => println!(
        //         "No path found in Bidirectional adjacency list BFS (after {:.2?}).",
        //         elapsed_bi_adj
        //     ),
        // }

        // println!("\nRunning BFS on CSR graph...");
        // let now = Instant::now();
        // let path_csr = bfs_csr(csr_graph, start_id, goal_id, max_depth);
        // let elapsed_csr = now.elapsed();

        // match path_csr {
        //     Some(path) => {
        //         println!(
        //             "Path found (CSR) [{} nodes, {:.2?}]:",
        //             path.len(),
        //             elapsed_csr
        //         );
        //         for id in path {
        //             let title = id_to_title
        //                 .get(&id)
        //                 .map(String::as_str)
        //                 .unwrap_or("[Unknown]");
        //             print!("{} -> ", title);
        //         }
        //         println!("END");
        //     }
        //     None => println!("No path found in CSR BFS (after {:.2?}).", elapsed_csr),
        // }

        // println!("\nRunning backwards BFS on csr...");
        // let now = Instant::now();
        // let path_adj = bfs_csr_backwards(csr_graph, start_id, goal_id, max_depth);
        // let elapsed_adj = now.elapsed();

        // match path_adj {
        //     Some(path) => {
        //         println!(
        //             "Path found (csr) [{} nodes, {:.2?}]:",
        //             path.len(),
        //             elapsed_adj
        //         );
        //         for id in path {
        //             let title = id_to_title
        //                 .get(&id)
        //                 .map(String::as_str)
        //                 .unwrap_or("[Unknown]");
        //             print!("{} -> ", title);
        //         }
        //         println!("END");
        //     }
        //     None => println!("No path found in csr BFS (after {:.2?}).", elapsed_adj),
        // }

        // // Bidirectional BFS csr
        // println!("\nRunning Bidirectional BFS on CSR graph...");
        // let now = Instant::now();
        // let path_bi_adj = bi_bfs_csr(csr_graph, start_id, goal_id, max_depth);
        // let elapsed_bi_adj = now.elapsed();
        // match path_bi_adj {
        //     Some(path) => {
        //         println!(
        //             "Path found (Bidirectional csr) [{} nodes, {:.2?}]:",
        //             path.len(),
        //             elapsed_bi_adj
        //         );
        //         for id in path {
        //             let title = id_to_title
        //                 .get(&id)
        //                 .map(String::as_str)
        //                 .unwrap_or("[Unknown]");
        //             print!("{} -> ", title);
        //         }
        //         println!("END");
        //     }
        //     None => println!(
        //         "No path found in Bidirectional csr BFS (after {:.2?}).",
        //         elapsed_bi_adj
        //     ),
        // }

        println!("\n----------------------------\n");
    }

    println!("Exiting interactive session.");
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

// Echinorhynchida -> Illiosentidae is not found even though Echinorhynchida -> Leptorhynchoididae is found
// Illiosentidae redirects to Leptorhynchoididae

// how should i handle the case where on a page (say banana) there are two links: one is a redirect to apple (fruit apple) and the other is a direct link to apple (apple fruit)
// do i consider banana -> fruit apple and banana -> apple fruit the same path or keep both
// i will keep only 1 for now
