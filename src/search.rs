use rustc_hash::FxHashMap;
use std::{
    collections::VecDeque,
    io::{self, Write},
    time::Instant,
};

use crate::parsers::pagelinks_parser;

// ilisendipede something -> Leptorhynchoididae (redirect to target)
pub fn bfs_adj_list(
    graph: &FxHashMap<u32, Vec<u32>>,
    redirect_targets: &FxHashMap<u32, u32>,
    orig_start: u32,
    orig_goal: u32,
    max_depth: u8,
) -> Option<Vec<u32>> {
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
        return Some(vec![orig_start]);
    }

    let mut queue = VecDeque::new();
    let mut parents = FxHashMap::default();
    // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
    // (page, redirect target): redirect   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
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
                    let neighbor =
                        if let Some(&redirect_target) = redirect_targets.get(&raw_neighbor) {
                            redirects_passed.insert((node, redirect_target), raw_neighbor);
                            redirect_target
                        } else {
                            raw_neighbor
                        };
                    if !parents.contains_key(&neighbor) {
                        parents.insert(neighbor, node);
                        if neighbor == goal {
                            let elapsed = now.elapsed();
                            println!("Elapsed: {:.2?}", elapsed);
                            return Some(reconstruct_path(
                                start,
                                goal,
                                &parents,
                                &redirects_passed,
                                true,
                            ));
                        }
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        depth += 1;
    }

    None
}

pub fn bfs_adj_list_backwards(
    graph: &FxHashMap<u32, Vec<u32>>,
    redirect_targets: &FxHashMap<u32, u32>,
    orig_start: u32,
    orig_goal: u32,
    max_depth: u8,
) -> Option<Vec<u32>> {
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
        return Some(vec![orig_goal]);
    }

    let mut queue = VecDeque::new();
    let mut parents = FxHashMap::default();
    // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
    // (page, redirect target): target   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
    let mut redirects_passed: FxHashMap<(u32, u32), u32> = FxHashMap::default();

    parents.insert(goal, goal); // mark start as visited
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
                                    parents.insert(redirect_neighbor, node);
                                    if redirect_neighbor == start {
                                        let elapsed = now.elapsed();
                                        println!("Elapsed: {:.2?}", elapsed);
                                        return Some(reconstruct_path_backwards(
                                            start,
                                            goal,
                                            &parents,
                                            &redirects_passed,
                                            true,
                                        ));
                                    }
                                    queue.push_back(redirect_neighbor);
                                }
                            }
                        }
                    } else {
                        if !parents.contains_key(&raw_neighbor) {
                            parents.insert(raw_neighbor, node);
                            if raw_neighbor == start {
                                let elapsed = now.elapsed();
                                println!("Elapsed: {:.2?}", elapsed);
                                return Some(reconstruct_path_backwards(
                                    start,
                                    goal,
                                    &parents,
                                    &redirects_passed,
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

pub fn bi_bfs_adj_list(
    graph_fwd: &FxHashMap<u32, Vec<u32>>,
    graph_bwd: &FxHashMap<u32, Vec<u32>>,
    redirect_targets: &FxHashMap<u32, u32>,
    orig_start: u32,
    orig_goal: u32,
    max_depth: u8,
) -> Option<Vec<u32>> {
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
        return Some(vec![orig_start]);
    }

    let mut queue_fwd = VecDeque::new();
    let mut queue_bwd = VecDeque::new();
    let mut parents_fwd = FxHashMap::default();
    let mut parents_bwd = FxHashMap::default();
    // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
    // (page, redirect target): redirect   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
    let mut redirects_passed_fwd: FxHashMap<(u32, u32), u32> = FxHashMap::default();
    let mut redirects_passed_bwd: FxHashMap<(u32, u32), u32> = FxHashMap::default();

    parents_fwd.insert(start, start); // mark start as visited
    parents_bwd.insert(goal, goal); // mark start as visited
    queue_fwd.push_back(start);
    queue_bwd.push_back(goal);

    let mut depth_fwd = 0;
    let mut depth_bwd = 0;

    while !queue_fwd.is_empty() && !queue_bwd.is_empty() {
        let (queue, parents_this, parents_other, redirects_passed, graph, fwd_smaller) =
            if queue_fwd.len() <= queue_bwd.len() {
                (
                    &mut queue_fwd,
                    &mut parents_fwd,
                    &mut parents_bwd,
                    &mut redirects_passed_fwd,
                    graph_fwd,
                    true,
                )
            } else {
                (
                    &mut queue_bwd,
                    &mut parents_bwd,
                    &mut parents_fwd,
                    &mut redirects_passed_bwd,
                    graph_bwd,
                    false,
                )
            };

        if fwd_smaller {
            depth_fwd += 1;
            println!("Depth forward {}", depth_fwd);
            if depth_fwd + depth_bwd >= max_depth {
                println!("MAX DEPTH REACHED");
                return None;
            }
        } else {
            depth_bwd += 1;
            println!("Depth backwards {}", depth_bwd);
            if depth_fwd + depth_bwd >= max_depth {
                println!("MAX DEPTH REACHED");
                return None;
            }
        }

        let level_size = queue.len();
        for _ in 0..level_size {
            let node = queue.pop_front().unwrap();

            if let Some(neighbors) = graph.get(&node) {
                for &raw_neighbor in neighbors {
                    if fwd_smaller {
                        // FORWARD
                        let neighbor =
                            if let Some(&redirect_target) = redirect_targets.get(&raw_neighbor) {
                                redirects_passed.insert((node, redirect_target), raw_neighbor);
                                redirect_target
                            } else {
                                raw_neighbor
                            };
                        if !parents_this.contains_key(&neighbor) {
                            parents_this.insert(neighbor, node);
                            // check for meeting point
                            if parents_other.contains_key(&neighbor) {
                                println!("Elapsed: {:.2?}", now.elapsed());
                                return Some(merge_paths(
                                    start,
                                    goal,
                                    neighbor,
                                    &parents_fwd,
                                    &parents_bwd,
                                    &redirects_passed_fwd,
                                    &redirects_passed_bwd,
                                ));
                            }
                            queue.push_back(neighbor);
                        }
                    } else {
                        // BACKWARDS
                        // if the neighbor is a redirect, add its neighbors to the queue instead
                        if redirect_targets.contains_key(&raw_neighbor) {
                            if let Some(redirect_neighbors) = graph.get(&raw_neighbor) {
                                for &redirect_neighbor in redirect_neighbors {
                                    redirects_passed
                                        .insert((redirect_neighbor, node), raw_neighbor);

                                    if !parents_this.contains_key(&redirect_neighbor) {
                                        parents_this.insert(redirect_neighbor, node);
                                        // check for meeting point
                                        if parents_other.contains_key(&redirect_neighbor) {
                                            println!("Elapsed: {:.2?}", now.elapsed());
                                            return Some(merge_paths(
                                                start,
                                                goal,
                                                redirect_neighbor,
                                                &parents_fwd,
                                                &parents_bwd,
                                                &redirects_passed_fwd,
                                                &redirects_passed_bwd,
                                            ));
                                        }
                                        queue.push_back(redirect_neighbor);
                                    }
                                }
                            }
                        } else {
                            if !parents_this.contains_key(&raw_neighbor) {
                                parents_this.insert(raw_neighbor, node);
                                // check for meeting point
                                if parents_other.contains_key(&raw_neighbor) {
                                    println!("Elapsed: {:.2?}", now.elapsed());
                                    return Some(merge_paths(
                                        start,
                                        goal,
                                        raw_neighbor,
                                        &parents_fwd,
                                        &parents_bwd,
                                        &redirects_passed_fwd,
                                        &redirects_passed_bwd,
                                    ));
                                }
                                queue.push_back(raw_neighbor);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn reconstruct_path(
    start: u32,
    goal: u32,
    parents: &FxHashMap<u32, u32>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    return_redirects: bool,
) -> Vec<u32> {
    // reconstruct path
    let mut path = Vec::new();
    let mut current = goal;
    loop {
        path.push(current);
        if current == start {
            break;
        }
        let &parent = parents.get(&current).unwrap();
        current = parent;
    }
    path.reverse();

    if !return_redirects {
        return path;
    };

    // turn the target back into the redirect that led it there
    let mut resolved_path = Vec::new();
    resolved_path.push(start);
    for window in path.windows(2) {
        let prev_node = window[0];
        let node = window[1];
        resolved_path.push(
            redirects_passed
                .get(&(prev_node, node))
                .copied()
                .unwrap_or(node),
        );
    }
    return resolved_path;
}

// when using incoming links
pub fn reconstruct_path_backwards(
    start: u32,
    goal: u32,
    parents: &FxHashMap<u32, u32>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    return_redirects: bool,
) -> Vec<u32> {
    // reconstruct path
    let mut path = Vec::new();
    let mut current = start;
    loop {
        path.push(current);
        if current == goal {
            break;
        }
        let &parent = parents.get(&current).unwrap();
        current = parent;
    }
    if !return_redirects {
        return path;
    };

    // turn the target back into the redirect that led it there
    let mut resolved_path = Vec::new();
    resolved_path.push(start);
    for window in path.windows(2) {
        let prev_node = window[0];
        let node = window[1];
        resolved_path.push(
            redirects_passed
                .get(&(prev_node, node))
                .copied()
                .unwrap_or(node),
        );
    }
    return resolved_path;
}

fn merge_paths(
    start: u32,
    goal: u32,
    meet: u32,
    parents_fwd: &FxHashMap<u32, u32>,
    parents_bwd: &FxHashMap<u32, u32>,
    redirects_fwd: &FxHashMap<(u32, u32), u32>,
    redirects_bwd: &FxHashMap<(u32, u32), u32>,
) -> Vec<u32> {
    let mut path_fwd = reconstruct_path(start, meet, parents_fwd, redirects_fwd, true);
    let path_bwd = reconstruct_path_backwards(meet, goal, parents_bwd, redirects_bwd, true);

    // do not pop from path_fwd.pop(); if the meet point is supposed to be a redirect, it only gets put back in path_fwd
    // because for path_bwd the meet point is at the front which doesn't get changes
    // ex: Forward path: [1613879 Plastic_bag, 70691392 Phase-out_of_lightweight_plastic_bags] Backward path: [36080727 Plastic_bag_ban]

    // remove duplicate meet point (first element of path_bwd)
    path_fwd.extend(&path_bwd[1..]);
    path_fwd
}

fn bfs_csr(
    graph: &pagelinks_parser::CsrGraph,
    orig_start: u32,
    orig_goal: u32,
    max_depth: u8,
) -> Option<Vec<u32>> {
    let now = Instant::now();

    let mut start = graph.orig_to_dense.get(&orig_start).copied()?;
    let mut goal = graph.orig_to_dense.get(&orig_goal).copied()?;

    // if start or goal is redirect, resolve it
    start = graph.resolve_redirect(start).unwrap_or(start);
    goal = graph.resolve_redirect(goal).unwrap_or(goal);

    // case where start is same as goal (can happen when the start is a redirect to the goal)
    if start == goal {
        return Some(vec![orig_start]);
    }

    let mut queue = VecDeque::new();
    let mut parents: FxHashMap<u32, u32> = FxHashMap::default();
    // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
    // (page, redirect target): redirect   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
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

            let neighbors = graph.get(node);
            for &raw_neighbor in neighbors {
                let neighbor = if let Some(redirect_target) = graph.resolve_redirect(raw_neighbor) {
                    redirects_passed.insert((node, redirect_target), raw_neighbor);
                    redirect_target
                } else {
                    raw_neighbor
                };

                if !parents.contains_key(&neighbor) {
                    parents.insert(neighbor, node);
                    if neighbor == goal {
                        let elapsed = now.elapsed();
                        println!("Elapsed: {:.2?}", elapsed);
                        return Some(reconstruct_path_csr(
                            start,
                            goal,
                            &parents,
                            &redirects_passed,
                            &graph.dense_to_orig,
                            true,
                        ));
                    }
                    queue.push_back(neighbor);
                }
            }
        }

        depth += 1;
    }

    None
}

pub fn reconstruct_path_csr(
    start: u32,
    goal: u32,
    parents: &FxHashMap<u32, u32>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    dense_to_orig: &Vec<u32>,
    return_redirects: bool,
) -> Vec<u32> {
    let path = reconstruct_path(start, goal, parents, redirects_passed, return_redirects);

    let orig_path: Vec<u32> = path
        .into_iter()
        .map(|node| dense_to_orig[node as usize])
        .collect();

    return orig_path;
}

pub fn bfs_interactive_session(
    title_to_id: &FxHashMap<String, u32>,
    id_to_title: &FxHashMap<u32, String>,
    csr_graph: &pagelinks_parser::CsrGraph,
    adj_graph: &FxHashMap<u32, Vec<u32>>,
    adj_graph_bwd: &FxHashMap<u32, Vec<u32>>,
    redirect_targets: &FxHashMap<u32, u32>,
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

        let Some(&start_id) = title_to_id.get(start_title) else {
            println!("Start title '{}' not found in mapping.", start_title);
            continue;
        };
        let Some(&goal_id) = title_to_id.get(goal_title) else {
            println!("Goal title '{}' not found in mapping.", goal_title);
            continue;
        };

        let max_depth = 50;

        println!("\nRunning BFS on adjacency list graph...");
        let now = Instant::now();
        let path_adj = bfs_adj_list(adj_graph, redirect_targets, start_id, goal_id, max_depth);
        let elapsed_adj = now.elapsed();

        match path_adj {
            Some(path) => {
                println!(
                    "Path found (adjacency list) [{} nodes, {:.2?}]:",
                    path.len(),
                    elapsed_adj
                );
                for id in path {
                    let title = id_to_title
                        .get(&id)
                        .map(String::as_str)
                        .unwrap_or("[Unknown]");
                    print!("{} -> ", title);
                }
                println!("END");
            }
            None => println!(
                "No path found in adjacency list BFS (after {:.2?}).",
                elapsed_adj
            ),
        }

        println!("\nRunning backwards BFS on adjacency list graph...");
        let now = Instant::now();
        let path_adj = bfs_adj_list_backwards(
            adj_graph_bwd,
            redirect_targets,
            start_id,
            goal_id,
            max_depth,
        );
        let elapsed_adj = now.elapsed();

        match path_adj {
            Some(path) => {
                println!(
                    "Path found (adjacency list) [{} nodes, {:.2?}]:",
                    path.len(),
                    elapsed_adj
                );
                for id in path {
                    let title = id_to_title
                        .get(&id)
                        .map(String::as_str)
                        .unwrap_or("[Unknown]");
                    print!("{} -> ", title);
                }
                println!("END");
            }
            None => println!(
                "No path found in adjacency list BFS (after {:.2?}).",
                elapsed_adj
            ),
        }

        // Bidirectional BFS
        println!("\nRunning Bidirectional BFS on adjacency list graph...");
        let now = Instant::now();
        let path_bi_adj = bi_bfs_adj_list(
            adj_graph,
            adj_graph_bwd,
            redirect_targets,
            start_id,
            goal_id,
            max_depth,
        );
        let elapsed_bi_adj = now.elapsed();
        match path_bi_adj {
            Some(path) => {
                println!(
                    "Path found (Bidirectional adjacency list) [{} nodes, {:.2?}]:",
                    path.len(),
                    elapsed_bi_adj
                );
                for id in path {
                    let title = id_to_title
                        .get(&id)
                        .map(String::as_str)
                        .unwrap_or("[Unknown]");
                    print!("{} -> ", title);
                }
                println!("END");
            }
            None => println!(
                "No path found in Bidirectional adjacency list BFS (after {:.2?}).",
                elapsed_bi_adj
            ),
        }

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
