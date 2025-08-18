use rustc_hash::FxHashMap;
use std::{
    collections::VecDeque,
    fs::File,
    io::{self, Write},
    time::Instant,
};

use crate::graph::*;
use crate::mmap_structs::*;

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
    redirects_passed: &RedirectsPassedMmap,
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
            redirects_passed,
            true,
            backwards,
        ));
    }

    None
}

pub fn bi_bfs_adj_list(
    graph_fwd: &FxHashMap<u32, Vec<u32>>,
    graph_bwd: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &RedirectsPassedMmap,
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
            redirects_passed,
            true,
        ));
    }

    None
}

pub fn reconstruct_all_paths(
    start: u32,
    goal: u32,
    parents: &FxHashMap<u32, Vec<u32>>,
    redirects_passed: &RedirectsPassedMmap,
    // redirects_passed: &FxHashMap<(u32, u32), u32>,
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
            resolved_path.push(redirects_passed.get(prev_node, node).unwrap_or(node));
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
    redirects_passed: &RedirectsPassedMmap,
    // redirects_passed: &FxHashMap<(u32, u32), u32>,
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

pub fn bfs_csr<G>(
    graph: &G,
    orig_start: u32,
    orig_goal: u32,
    max_depth: u8,
    backwards: bool,
    redirects_passed: &RedirectsPassedMmap,
    // redirects_passed: &FxHashMap<(u32, u32), u32>,
) -> Option<Vec<Vec<u32>>>
where
    G: CsrGraphTrait,
{
    let now = Instant::now();

    // DO NOT PASS IN REDIRECTS AS START AND GOAL
    let (start, goal) = if backwards {
        (orig_goal, orig_start)
    } else {
        (orig_start, orig_goal)
    };

    // start and goal should be dense ids, also should not be redirects

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
        return Some(reconstruct_all_paths(
            start,
            goal,
            &parents,
            redirects_passed,
            true,
            backwards,
        ));
    }

    None
}

pub fn bi_bfs_csr<G>(
    graph: &G,
    start: u32,
    goal: u32,
    max_depth: u8,
    redirects_passed: &RedirectsPassedMmap,
    // redirects_passed: &FxHashMap<(u32, u32), u32>,
) -> Option<Vec<Vec<u32>>>
where
    G: CsrGraphTrait,
{
    let now = Instant::now();

    // start and goal should be dense ids, also should not be redirects

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
        println!("search done in: {:.2?}", elapsed);

        return Some(merge_all_paths(
            start,
            &meet_nodes,
            goal,
            &parents_fwd,
            &parents_bwd,
            redirects_passed,
            true,
        ));
    }

    None
}

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

pub fn bfs_interactive_session<G>(
    // title_to_id: &FxHashMap<String, u32>,
    // id_to_title: &FxHashMap<u32, String>,
    title_to_dense_id: &crate::TitleToDenseIdMmap,
    dense_id_to_title: &crate::DenseIdToTitleMmap,
    csr_graph: &G,
    // adj_graph: &FxHashMap<u32, Vec<u32>>,
    // adj_graph_bwd: &FxHashMap<u32, Vec<u32>>,
    // redirect_targets: &FxHashMap<u32, u32>,
    dense_redirect_targets: &RedirectTargetsDenseMmap,
    // dense_redirect_targets: &FxHashMap<u32, u32>,
    redirects_passed: &RedirectsPassedMmap,
    // redirects_passed: &FxHashMap<(u32, u32), u32>,
) where
    G: CsrGraphTrait,
{
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

        let Some(start_id_raw) = title_to_dense_id.get(start_title) else {
            println!("Start title '{}' not found in mapping.", start_title);
            continue;
        };
        let Some(goal_id_raw) = title_to_dense_id.get(goal_title) else {
            println!("Goal title '{}' not found in mapping.", goal_title);
            continue;
        };

        let mut start_id = start_id_raw;
        let mut goal_id = goal_id_raw;

        // Resolve redirects
        let redirect = dense_redirect_targets.get(start_id);
        if redirect != u32::MAX {
            start_id = redirect;
        }

        let redirect = dense_redirect_targets.get(goal_id);
        if redirect != u32::MAX {
            goal_id = redirect;
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
        let paths_adj_fwd = bi_bfs_csr(csr_graph, start_id, goal_id, max_depth, redirects_passed);
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
                    let title = dense_id_to_title.get(*id);
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

use rand::Rng;
pub fn benchmark_random_bfs<G>(
    graph: &G,
    redirect_targets_dense: &FxHashMap<u32, u32>,
    num_pairs: usize,
    max_depth: u8,
    redirects_passed_mmap: &RedirectsPassedMmap,
    // redirects_passed_mmap: &FxHashMap<(u32, u32), u32>,
) where
    G: CsrGraphTrait,
{
    let start_time = Instant::now();
    let mut times: Vec<f64> = Vec::with_capacity(num_pairs);

    let mut rng = rand::rng();
    let n = graph.num_nodes();

    for _ in 0..num_pairs {
        let start_dense = rng.random_range(0..n) as u32;
        let goal_dense = rng.random_range(0..n) as u32;

        // Resolve redirects
        let start_id = redirect_targets_dense
            .get(&start_dense)
            .copied()
            .unwrap_or(start_dense);
        let goal_id = redirect_targets_dense
            .get(&goal_dense)
            .copied()
            .unwrap_or(goal_dense);

        println!("\nRunning BFS: {} -> {}", start_id, goal_dense);

        let start_time = Instant::now();
        let paths = bi_bfs_csr::<G>(graph, start_id, goal_id, max_depth, redirects_passed_mmap);
        let elapsed = start_time.elapsed();
        times.push(elapsed.as_secs_f64());

        match paths {
            Some(p) => println!("Found {} paths in {:.2?}", p.len(), elapsed),
            None => println!("No path found (elapsed {:.2?})", elapsed),
        }
    }
    let elapsed = start_time.elapsed();
    println!(
        "Benchmark for {} pairs,  (elapsed {:.2?})",
        num_pairs, elapsed
    );

    // Save times to CSV
    let mut file = File::create("bfs_times.csv").unwrap();
    for t in &times {
        writeln!(file, "{:.6}", t).unwrap();
    }
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

// Takizawa_Bakin -> Phase-out_of_lightweight_plastic_bags
// Lunitidal_interval -> Length_(phonetics)"

// find sources and sinks and paths between them
