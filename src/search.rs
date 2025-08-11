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
    mut start: u32,
    mut goal: u32,
    max_depth: u8,
) -> Option<Vec<u32>> {
    let now = Instant::now();

    start = redirect_targets.get(&start).copied().unwrap_or(start);
    goal = redirect_targets.get(&goal).copied().unwrap_or(goal);

    let mut queue = VecDeque::new();
    let mut parents = FxHashMap::default();
    // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
    // (redirect target, page): target   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
    let mut redirects_passed: FxHashMap<(u32, u32), u32> = FxHashMap::default();

    // case where start is same as goal (can happen when the start is a redirect to the goal)
    if start == goal {
        return Some(reconstruct_path(start, goal, &parents, &redirects_passed));
    }

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

// BUGGED
// maybe make a converted redirect map cause maybe thats why its slow
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

    let mut queue = VecDeque::new();
    let mut parents: FxHashMap<u32, u32> = FxHashMap::default();
    // if you encounter a neighbor on a page that is a redirect, add the resolved redirect target to the frontier, but also add the redirect to the map
    // (redirect target, page): target   ----   so when you rebuild the path you can change it back into the redirect that was found on that page
    let mut redirects_passed: FxHashMap<(u32, u32), u32> = FxHashMap::default();

    // case where start is same as goal (can happen when the start is a redirect to the goal)
    if start == goal {
        return Some(reconstruct_path_csr(
            start,
            goal,
            &parents,
            &redirects_passed,
            &graph.dense_to_orig,
        ));
    }

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
                let mut neighbor = raw_neighbor;

                if let Some(redirect_target) = graph.resolve_redirect(neighbor) {
                    redirects_passed.insert((redirect_target, node), neighbor);
                    neighbor = redirect_target;
                }
                if !parents.contains_key(&neighbor) {
                    parents.insert(neighbor, node);
                    queue.push_back(neighbor);
                    if neighbor == goal {
                        let elapsed = now.elapsed();
                        println!("Elapsed: {:.2?}", elapsed);
                        return Some(reconstruct_path_csr(
                            start,
                            goal,
                            &parents,
                            &redirects_passed,
                            &graph.dense_to_orig,
                        ));
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

pub fn reconstruct_path_csr(
    start: u32,
    goal: u32,
    parents: &FxHashMap<u32, u32>,
    redirects_passed: &FxHashMap<(u32, u32), u32>,
    dense_to_orig: &Vec<u32>,
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
    resolved_path.push(path[0]);
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

    let orig_path: Vec<u32> = resolved_path
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

        println!("\nRunning BFS on CSR graph...");
        let now = Instant::now();
        let path_csr = bfs_csr(csr_graph, start_id, goal_id, max_depth);
        let elapsed_csr = now.elapsed();

        match path_csr {
            Some(path) => {
                println!(
                    "Path found (CSR) [{} nodes, {:.2?}]:",
                    path.len(),
                    elapsed_csr
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
            None => println!("No path found in CSR BFS (after {:.2?}).", elapsed_csr),
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
