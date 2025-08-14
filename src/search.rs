use rustc_hash::FxHashMap;
use std::{
    collections::VecDeque,
    io::{self, Write},
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

fn process_neighbor_bi(
    neighbor: u32,
    next_depth: u8,
    next_combined_depth: u8,
    meet_nodes: &mut Vec<u32>,
    visited_depth_this: &mut FxHashMap<u32, u8>,
    visited_depth_other: &mut FxHashMap<u32, u8>,
    parents: &mut FxHashMap<u32, Vec<u32>>,
    node: u32,
    meet_found_at_depth: &mut Option<u8>,
    queue: &mut VecDeque<(u32, u8)>,
) {
    match visited_depth_this.get(&neighbor) {
        // Case 1: Never seen this neighbor before. This is a valid new path.
        None => {
            visited_depth_this.insert(neighbor, next_depth);
            parents.insert(neighbor, vec![node]);
            queue.push_back((neighbor, next_depth)); // now we can push here directly
        }
        // Case 2: Seen this neighbor before AT THE SAME DEPTH. This is a valid parallel path.
        Some(&depth) if depth == next_depth => {
            parents.get_mut(&neighbor).unwrap().push(node);
        }
        // Case 3: Seen this neighbor before at an earlier depth. This is a longer path or a cycle. Ignore it.
        _ => (),
    }

    // check for meeting point
    if visited_depth_other.contains_key(&neighbor) {
        if meet_found_at_depth.is_none() {
            *meet_found_at_depth = Some(next_combined_depth);
            println!("path found at depth {}", next_combined_depth);
        }
        // this check prevents duplicates in meet_nodes
        if !meet_nodes.contains(&neighbor) {
            meet_nodes.push(neighbor);
        }
    }
}

// ilisendipede something -> Leptorhynchoididae (redirect to target)
pub fn bfs_adj_list(
    graph: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    orig_start: u32,
    orig_goal: u32,
    max_depth: u8,
    backwards: bool,
) -> Option<Vec<Vec<u32>>> {
    let now = Instant::now();

    // DO NOT PASS IN REDIRECTS AS START AND GOAL

    let (start, goal) = if backwards {
        (orig_goal, orig_start)
    } else {
        (orig_start, orig_goal)
    };

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
    let mut depth = 0;

    while !queue.is_empty() {
        // If we have already found the goal, finish this depth and then stop
        if let Some(goal_depth) = goal_found_at_depth {
            if depth >= goal_depth {
                break;
            }
        }

        // Check max depth
        if depth >= max_depth {
            println!("MAX DEPTH REACHED");
            return None;
        }

        depth += 1;
        println!("Depth {}", depth);

        let level_size = queue.len();
        for _ in 0..level_size {
            let (node, current_depth) = queue.pop_front().unwrap();
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
            backwards,
        ));
    }

    None
}

pub fn bi_bfs_adj_list(
    graph_fwd: &FxHashMap<u32, Vec<u32>>,
    graph_bwd: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    start: u32,
    goal: u32,
    max_depth: u8,
) -> Option<Vec<Vec<u32>>> {
    let now = Instant::now();

    // case where start is same as goal (can happen when the start is a redirect to the goal)
    if start == goal {
        return Some(vec![vec![start]]);
    }

    let mut queue_fwd = VecDeque::new();
    let mut queue_bwd = VecDeque::new();
    let mut parents_fwd: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut parents_bwd: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut visited_depth_fwd: FxHashMap<u32, u8> = FxHashMap::default();
    let mut visited_depth_bwd: FxHashMap<u32, u8> = FxHashMap::default();

    visited_depth_fwd.insert(start, 0);
    visited_depth_bwd.insert(goal, 0);

    queue_fwd.push_back((start, 0));
    queue_bwd.push_back((goal, 0));

    let mut meet_nodes: Vec<u32> = Vec::new();
    let mut meet_found_at_depth: Option<u8> = None;

    let mut depth_fwd = 0;
    let mut depth_bwd = 0;

    while !queue_fwd.is_empty() && !queue_bwd.is_empty() {
        let combined_depth = depth_fwd + depth_bwd;

        if let Some(meet_depth) = meet_found_at_depth {
            if combined_depth >= meet_depth {
                break;
            }
        }
        // Check max depth
        if combined_depth >= max_depth {
            println!("MAX DEPTH REACHED");
            return None;
        }

        let (queue, parents, visited_depth_this, visited_depth_other, depth, graph, backwards) =
            if queue_fwd.len() <= queue_bwd.len() {
                (
                    &mut queue_fwd,
                    &mut parents_fwd,
                    &mut visited_depth_fwd,
                    &mut visited_depth_bwd,
                    &mut depth_fwd,
                    graph_fwd,
                    false,
                )
            } else {
                (
                    &mut queue_bwd,
                    &mut parents_bwd,
                    &mut visited_depth_bwd,
                    &mut visited_depth_fwd,
                    &mut depth_bwd,
                    graph_bwd,
                    true,
                )
            };

        *depth += 1;
        println!(
            "Depth {} {}",
            if backwards { "backwards" } else { "forwards" },
            depth
        );

        let level_size = queue.len();
        for _ in 0..level_size {
            let (node, current_depth) = queue.pop_front().unwrap();
            let next_depth = current_depth + 1;
            let next_combined_depth = combined_depth + 1;

            if let Some(neighbors) = graph.get(&node) {
                for &neighbor in neighbors {
                    process_neighbor_bi(
                        neighbor,
                        next_depth,
                        next_combined_depth,
                        &mut meet_nodes,
                        visited_depth_this,
                        visited_depth_other,
                        parents,
                        node,
                        &mut meet_found_at_depth,
                        queue,
                    );
                }
            }
        }
    }

    if meet_found_at_depth.is_some() {
        let elapsed = now.elapsed();
        println!("Elapsed: {:.2?}", elapsed);

        return Some(merge_all_paths(
            start,
            &meet_nodes,
            goal,
            &parents_fwd,
            &parents_bwd,
            &redirects_passed,
            true,
        ));
    }

    None
}

pub fn reconstruct_all_paths(
    start: u32,
    goal: u32,
    parents: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    return_redirects: bool,
    reverse: bool,
) -> Vec<Vec<u32>> {
    let mut all_paths = Vec::new();

    // working path buffer: stores nodes in reverse order: goal, parent, parent, ..., current
    let mut path: Vec<u32> = Vec::new();
    path.push(goal);

    // Stack of frames: (&parents_slice_for_node, next_index_to_try)
    // We store slices (&[u32]) borrowed from the `parents` map so we avoid repeated map lookups.
    let mut stack: Vec<(&[u32], usize)> = Vec::new();
    let goal_pars: &[u32] = parents.get(&goal).map(|v| v.as_slice()).unwrap_or(&[]);
    stack.push((goal_pars, 0));

    while let Some((pars, idx)) = stack.last_mut() {
        // Current node is always the last element in `path`.
        let current = *path.last().unwrap();

        // If we've reached start, materialize a path (start -> ... -> goal)
        if current == start {
            let mut out = Vec::with_capacity(path.len());
            // path is goal..start (end is start), so iterate reversed to get start..goal
            for &n in path.iter().rev() {
                out.push(n);
            }
            all_paths.push(out);

            // backtrack: pop frame and last node
            stack.pop();
            path.pop();
            continue;
        }

        // If there are still parents to explore for current node, descend to next parent
        if *idx < pars.len() {
            let p = pars[*idx];
            *idx += 1;

            // push parent into the path and push its parents slice onto the stack
            path.push(p);
            let p_pars: &[u32] = parents.get(&p).map(|v| v.as_slice()).unwrap_or(&[]);
            stack.push((p_pars, 0));
        } else {
            // no more parents to try => backtrack
            stack.pop();
            path.pop();
        }
    }
    // // Stack holds (current_node, current_path)
    // // current_path is goal->...->current_node order
    // let mut stack: Vec<(u32, Vec<u32>)> = Vec::new();
    // stack.push((goal, vec![goal]));

    // while let Some((node, path)) = stack.pop() {
    //     if node == start {
    //         let mut complete_path = path.clone();
    //         complete_path.reverse(); // make it start->...->goal
    //         all_paths.push(complete_path);
    //     } else if let Some(pars) = parents.get(&node) {
    //         for &p in pars {
    //             let mut new_path = path.clone();
    //             new_path.push(p);
    //             stack.push((p, new_path));
    //         }
    //     }
    // }

    // If reverse flag is set, reverse each found path back
    if reverse {
        for path in &mut all_paths {
            path.reverse();
        }
    }

    if !return_redirects {
        return all_paths;
    }

    // Apply redirect resolution
    let mut resolved_paths = Vec::new();
    for path in all_paths {
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

    // there shouldn't be any duplicates
    resolved_paths
}

pub fn merge_all_paths(
    start: u32,
    meet_nodes: &Vec<u32>,
    goal: u32,
    parents_fwd: &FxHashMap<u32, Vec<u32>>,
    parents_bwd: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    return_redirects: bool,
) -> Vec<Vec<u32>> {
    let mut final_paths: Vec<Vec<u32>> = Vec::new();

    for &meet in meet_nodes {
        let fwd_parts = reconstruct_all_paths(
            start,
            meet,
            parents_fwd,
            redirects_passed,
            return_redirects,
            false,
        );
        let bwd_parts = reconstruct_all_paths(
            goal,
            meet,
            parents_bwd,
            redirects_passed,
            return_redirects,
            true,
        );

        for fwd_part in &fwd_parts {
            for bwd_part in &bwd_parts {
                let mut combined = fwd_part.clone();
                // skip the meet from backward part because if the meet is a redirect
                // foward part's meet will correctly be a redirect while backward's meet will not (since it's the first element)
                combined.extend_from_slice(&bwd_part[1..]);
                final_paths.push(combined)
            }
        }
    }

    final_paths
}

pub fn bfs_csr(
    graph: &pagelinks_parser::CsrGraph,
    orig_start: u32,
    orig_goal: u32,
    max_depth: u8,
    backwards: bool,
) -> Option<Vec<Vec<u32>>> {
    let now = Instant::now();

    // DO NOT PASS IN REDIRECTS AS START AND GOAL
    let (mut start, mut goal) = if backwards {
        (orig_goal, orig_start)
    } else {
        (orig_start, orig_goal)
    };

    start = graph.orig_to_dense.get(&start).copied()?;
    goal = graph.orig_to_dense.get(&goal).copied()?;

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
    let mut depth = 0;

    while !queue.is_empty() {
        // If we have already found the goal, finish this depth and then stop
        if let Some(goal_depth) = goal_found_at_depth {
            if depth >= goal_depth {
                break;
            }
        }

        // Check max depth
        if depth >= max_depth {
            println!("MAX DEPTH REACHED");
            return None;
        }

        depth += 1;
        println!("Depth {}", depth);

        let level_size = queue.len();
        for _ in 0..level_size {
            let (node, current_depth) = queue.pop_front().unwrap();
            let next_depth = current_depth + 1;

            let neighbors = if backwards {
                graph.get_reverse(node)
            } else {
                graph.get(node)
            };

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
        return Some(reconstruct_all_paths_csr(
            start,
            goal,
            &parents,
            &graph.redirects_passed,
            &graph.dense_to_orig,
            true,
            backwards,
        ));
    }

    None
}

pub fn bi_bfs_csr(
    graph: &pagelinks_parser::CsrGraph,
    orig_start: u32,
    orig_goal: u32,
    max_depth: u8,
) -> Option<Vec<Vec<u32>>> {
    let now = Instant::now();

    let start = graph.orig_to_dense.get(&orig_start).copied()?;
    let goal = graph.orig_to_dense.get(&orig_goal).copied()?;

    // case where start is same as goal (can happen when the start is a redirect to the goal)
    if start == goal {
        return Some(vec![vec![start]]);
    }

    let mut queue_fwd = VecDeque::new();
    let mut queue_bwd = VecDeque::new();
    let mut parents_fwd: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut parents_bwd: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
    let mut visited_depth_fwd: FxHashMap<u32, u8> = FxHashMap::default();
    let mut visited_depth_bwd: FxHashMap<u32, u8> = FxHashMap::default();

    visited_depth_fwd.insert(start, 0);
    visited_depth_bwd.insert(goal, 0);

    queue_fwd.push_back((start, 0));
    queue_bwd.push_back((goal, 0));

    let mut meet_nodes: Vec<u32> = Vec::new();
    let mut meet_found_at_depth: Option<u8> = None;

    let mut depth_fwd = 0;
    let mut depth_bwd = 0;

    while !queue_fwd.is_empty() && !queue_bwd.is_empty() {
        let combined_depth = depth_fwd + depth_bwd;

        if let Some(meet_depth) = meet_found_at_depth {
            if combined_depth >= meet_depth {
                break;
            }
        }
        // Check max depth
        if combined_depth >= max_depth {
            println!("MAX DEPTH REACHED");
            return None;
        }

        let (queue, parents, visited_depth_this, visited_depth_other, depth, backwards) =
            if queue_fwd.len() <= queue_bwd.len() {
                (
                    &mut queue_fwd,
                    &mut parents_fwd,
                    &mut visited_depth_fwd,
                    &mut visited_depth_bwd,
                    &mut depth_fwd,
                    false,
                )
            } else {
                (
                    &mut queue_bwd,
                    &mut parents_bwd,
                    &mut visited_depth_bwd,
                    &mut visited_depth_fwd,
                    &mut depth_bwd,
                    true,
                )
            };

        *depth += 1;
        println!(
            "Depth {} {}",
            if backwards { "backwards" } else { "forwards" },
            depth
        );

        let level_size = queue.len();
        for _ in 0..level_size {
            let (node, current_depth) = queue.pop_front().unwrap();
            let next_depth = current_depth + 1;
            let next_combined_depth = combined_depth + 1;

            let neighbors = if backwards {
                graph.get_reverse(node)
            } else {
                graph.get(node)
            };

            for &neighbor in neighbors {
                process_neighbor_bi(
                    neighbor,
                    next_depth,
                    next_combined_depth,
                    &mut meet_nodes,
                    visited_depth_this,
                    visited_depth_other,
                    parents,
                    node,
                    &mut meet_found_at_depth,
                    queue,
                );
            }
        }
    }

    if meet_found_at_depth.is_some() {
        let elapsed = now.elapsed();
        println!("Elapsed: {:.2?}", elapsed);

        return Some(merge_all_paths_csr(
            start,
            &meet_nodes,
            goal,
            &parents_fwd,
            &parents_bwd,
            &graph.redirects_passed,
            &graph.dense_to_orig,
            true,
        ));
    }

    None
}

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

pub fn reconstruct_all_paths_csr(
    start: u32,
    goal: u32,
    parents: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    dense_to_orig: &Vec<u32>,
    return_redirects: bool,
    reverse: bool,
) -> Vec<Vec<u32>> {
    let all_paths = reconstruct_all_paths(
        start,
        goal,
        parents,
        redirects_passed,
        return_redirects,
        reverse,
    );

    let orig_all_paths = all_paths
        .into_iter()
        .map(|path| {
            path.into_iter()
                .map(|node| dense_to_orig[node as usize])
                .collect()
        })
        .collect();

    return orig_all_paths;
}

pub fn merge_all_paths_csr(
    start: u32,
    meet_nodes: &Vec<u32>,
    goal: u32,
    parents_fwd: &FxHashMap<u32, Vec<u32>>,
    parents_bwd: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    dense_to_orig: &Vec<u32>,
    return_redirects: bool,
) -> Vec<Vec<u32>> {
    let all_paths = merge_all_paths(
        start,
        meet_nodes,
        goal,
        parents_fwd,
        parents_bwd,
        redirects_passed,
        return_redirects,
    );

    let orig_all_paths = all_paths
        .into_iter()
        .map(|path| {
            path.into_iter()
                .map(|node| dense_to_orig[node as usize])
                .collect()
        })
        .collect();

    return orig_all_paths;
}

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
    // adj_graph: &FxHashMap<u32, Vec<u32>>,
    // adj_graph_bwd: &FxHashMap<u32, Vec<u32>>,
    redirect_targets: &FxHashMap<u32, u32>,
    // redirects_passed: &FxHashMap<(u32, u32), u32>,
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

        // println!("\nRunning forwards BFS on adjacency list graph...");
        // let now = Instant::now();
        // let paths_adj_fwd = bfs_adj_list(
        //     adj_graph,
        //     redirects_passed,
        //     start_id,
        //     goal_id,
        //     max_depth,
        //     false,
        // );
        // let elapsed_fwd = now.elapsed();

        // match &paths_adj_fwd {
        //     Some(paths) if !paths.is_empty() => {
        //         println!(
        //             "Paths found (adjacency list) [{} shortest paths, {:.2?}]:",
        //             paths.len(),
        //             elapsed_fwd
        //         );
        //         let path = &paths[0];
        //         println!("Path 1 ({} nodes):", path.len());
        //         for id in path {
        //             let title = id_to_title
        //                 .get(id)
        //                 .map(String::as_str)
        //                 .unwrap_or("[Unknown]");
        //             print!("{} -> ", title);
        //         }
        //         println!("END");
        //     }
        //     _ => println!(
        //         "No path found in adjacency list BFS (after {:.2?}).",
        //         elapsed_fwd
        //     ),
        // }

        // println!("\nRunning backwards BFS on adjacency list graph...");
        // let now = Instant::now();
        // let paths_adj_bwd = bfs_adj_list(
        //     adj_graph_bwd,
        //     redirects_passed,
        //     start_id,
        //     goal_id,
        //     max_depth,
        //     true,
        // );
        // let elapsed_bwd = now.elapsed();

        // match &paths_adj_bwd {
        //     Some(paths) if !paths.is_empty() => {
        //         println!(
        //             "Paths found (adjacency list) [{} shortest paths, {:.2?}]:",
        //             paths.len(),
        //             elapsed_bwd
        //         );
        //         let path = &paths[0];
        //         println!("Path 1 ({} nodes):", path.len());
        //         for id in path {
        //             let title = id_to_title
        //                 .get(id)
        //                 .map(String::as_str)
        //                 .unwrap_or("[Unknown]");
        //             print!("{} -> ", title);
        //         }
        //         println!("END");
        //     }
        //     _ => println!(
        //         "No path found in adjacency list BFS (after {:.2?}).",
        //         elapsed_bwd
        //     ),
        // }
        // if let (Some(fwd), Some(bwd)) = (paths_adj_fwd.clone(), paths_adj_bwd.clone()) {
        //     let fwd_set = paths_to_strings(&fwd, id_to_title);
        //     let bwd_set = paths_to_strings(&bwd, id_to_title);

        //     println!("\nForward BFS found {} shortest paths", fwd_set.len());
        //     println!("Backward BFS found {} shortest paths", bwd_set.len());

        //     let only_in_fwd: HashSet<_> = fwd_set.difference(&bwd_set).collect();
        //     let only_in_bwd: HashSet<_> = bwd_set.difference(&fwd_set).collect();

        //     println!("Paths only in forward BFS: {}", only_in_fwd.len());
        //     print_path_examples(
        //         &only_in_fwd.iter().cloned().cloned().collect(),
        //         "Only in Forward",
        //         10,
        //     );

        //     println!("Paths only in backward BFS: {}", only_in_bwd.len());
        //     print_path_examples(
        //         &only_in_bwd.iter().cloned().cloned().collect(),
        //         "Only in Backward",
        //         10,
        //     );
        // } else {
        //     println!("One or both BFS searches did not find paths, skipping comparison.");
        // }

        // println!("\nRunning bidirectional BFS on adjacency list graph...");
        // let now = Instant::now();
        // let paths_adj_bwd = bi_bfs_adj_list(
        //     adj_graph,
        //     adj_graph_bwd,
        //     redirects_passed,
        //     start_id,
        //     goal_id,
        //     max_depth,
        // );
        // let elapsed_bwd = now.elapsed();

        // match &paths_adj_bwd {
        //     Some(paths) if !paths.is_empty() => {
        //         println!(
        //             "Paths found (adjacency list) [{} shortest paths, {:.2?}]:",
        //             paths.len(),
        //             elapsed_bwd
        //         );
        //         let path = &paths[0];
        //         println!("Path 1 ({} nodes):", path.len());
        //         for id in path {
        //             let title = id_to_title
        //                 .get(id)
        //                 .map(String::as_str)
        //                 .unwrap_or("[Unknown]");
        //             print!("{} -> ", title);
        //         }
        //         println!("END");
        //     }
        //     _ => println!(
        //         "No path found in adjacency list BFS (after {:.2?}).",
        //         elapsed_bwd
        //     ),
        // }

        // println!("\nRunning forwards BFS on CSR graph...");
        // let now = Instant::now();
        // let paths_adj_fwd = bfs_csr(csr_graph, start_id, goal_id, max_depth, false);
        // let elapsed_fwd = now.elapsed();

        // match &paths_adj_fwd {
        //     Some(paths) if !paths.is_empty() => {
        //         println!(
        //             "Paths found (csr) [{} shortest paths, {:.2?}]:",
        //             paths.len(),
        //             elapsed_fwd
        //         );
        //         let path = &paths[0];
        //         println!("Path 1 ({} nodes):", path.len());
        //         for id in path {
        //             let title = id_to_title
        //                 .get(id)
        //                 .map(String::as_str)
        //                 .unwrap_or("[Unknown]");
        //             print!("{} -> ", title);
        //         }
        //         println!("END");
        //     }
        //     _ => println!("No path found in CSR BFS (after {:.2?}).", elapsed_fwd),
        // }

        // println!("\nRunning backwards BFS on CSR graph...");
        // let now = Instant::now();
        // let paths_adj_fwd = bfs_csr(csr_graph, start_id, goal_id, max_depth, true);
        // let elapsed_fwd = now.elapsed();

        // match &paths_adj_fwd {
        //     Some(paths) if !paths.is_empty() => {
        //         println!(
        //             "Paths found (csr) [{} shortest paths, {:.2?}]:",
        //             paths.len(),
        //             elapsed_fwd
        //         );
        //         let path = &paths[0];
        //         println!("Path 1 ({} nodes):", path.len());
        //         for id in path {
        //             let title = id_to_title
        //                 .get(id)
        //                 .map(String::as_str)
        //                 .unwrap_or("[Unknown]");
        //             print!("{} -> ", title);
        //         }
        //         println!("END");
        //     }
        //     _ => println!("No path found in CSR BFS (after {:.2?}).", elapsed_fwd),
        // }

        println!("\nRunning bidirectional BFS on CSR graph...");
        let now = Instant::now();
        let paths_adj_fwd = bi_bfs_csr(csr_graph, start_id, goal_id, max_depth);
        let elapsed_fwd = now.elapsed();

        match &paths_adj_fwd {
            Some(paths) if !paths.is_empty() => {
                println!(
                    "Paths found (csr) [{} shortest paths, {:.2?}]:",
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
            _ => println!("No path found in CSR BFS (after {:.2?}).", elapsed_fwd),
        }

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
