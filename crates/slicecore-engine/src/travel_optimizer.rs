//! TSP-based travel move optimization for toolpath ordering.
//!
//! This module implements several Travelling Salesman Problem (TSP) heuristics
//! to reorder printable elements within a layer, minimizing non-extrusion travel
//! distance. The key algorithms are:
//!
//! - **Nearest-neighbor (NN)** construction: greedy selection of the closest
//!   unvisited node at each step.
//! - **Greedy edge insertion**: build a tour by adding shortest edges that do not
//!   create premature cycles (using union-find).
//! - **2-opt local search**: iterative improvement by reversing sub-tours when
//!   doing so shortens total distance.
//!
//! The [`optimize_tour`] function is the main entry point; it dispatches to the
//! configured algorithm via [`TravelOptConfig`].

use slicecore_math::Point2;

use crate::config::{TravelOptAlgorithm, TravelOptConfig};

/// A node in the TSP problem representing a printable element.
///
/// Each node has an entry point (where the nozzle arrives) and an exit point
/// (where the nozzle is after printing). For closed paths (perimeters), entry
/// and exit are the same; for open paths (infill lines), they differ.
#[derive(Debug, Clone)]
pub struct TspNode {
    /// Entry point (where the nozzle arrives to start printing).
    pub entry: Point2,
    /// Exit point (where the nozzle is after printing this element).
    pub exit: Point2,
    /// Whether this path can be reversed (open paths like infill lines).
    pub reversible: bool,
    /// Index back to the original element for reordering.
    pub original_index: usize,
}

/// A tour (ordering) of TSP nodes.
///
/// Stores the order in which nodes should be visited and whether each node
/// should be printed in reverse direction (only meaningful for reversible nodes).
#[derive(Debug, Clone)]
pub struct Tour {
    /// Ordered indices into the `TspNode` slice.
    order: Vec<usize>,
    /// Whether each node in the tour is reversed (only meaningful for reversible nodes).
    reversed: Vec<bool>,
}

impl Tour {
    /// Creates a new tour with the given order, all nodes in forward direction.
    fn new(order: Vec<usize>) -> Self {
        let n = order.len();
        Self {
            order,
            reversed: vec![false; n],
        }
    }

    /// Returns the number of nodes in the tour.
    #[must_use]
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Returns `true` if the tour contains no nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Computes total travel distance for this tour (exit-to-entry between
    /// consecutive nodes).
    fn total_distance(&self, nodes: &[TspNode]) -> f64 {
        if self.order.len() <= 1 {
            return 0.0;
        }
        let mut total = 0.0;
        for i in 0..self.order.len() - 1 {
            total += self.edge_distance(nodes, i, i + 1);
        }
        total
    }

    /// Distance from tour position `i`'s exit to tour position `j`'s entry,
    /// accounting for reversal flags.
    fn edge_distance(
        &self,
        nodes: &[TspNode],
        i: usize,
        j: usize,
    ) -> f64 {
        let from = self.order[i];
        let to = self.order[j];
        let from_reversed = self.reversed[i];
        let to_reversed = self.reversed[j];

        let from_exit = if from_reversed && nodes[from].reversible {
            &nodes[from].entry
        } else {
            &nodes[from].exit
        };
        let to_entry = if to_reversed && nodes[to].reversible {
            &nodes[to].exit
        } else {
            &nodes[to].entry
        };

        // Direct distance computation (matrix stores non-reversed distances only).
        let dx = from_exit.x - to_entry.x;
        let dy = from_exit.y - to_entry.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Converts tour to output format: `Vec<(original_index, reversed)>`.
    #[must_use]
    pub fn to_permutation(&self, nodes: &[TspNode]) -> Vec<(usize, bool)> {
        self.order
            .iter()
            .zip(&self.reversed)
            .map(|(&idx, &rev)| {
                let actually_reversed = rev && nodes[idx].reversible;
                (nodes[idx].original_index, actually_reversed)
            })
            .collect()
    }
}

/// Precomputed distance matrix for TSP nodes.
///
/// Stores distances from the exit of node `i` to the entry of node `j`,
/// in both normal and reversed-entry orientations.
struct DistanceMatrix {
    n: usize,
    /// `dist[i * n + j]` = distance from exit of node `i` to entry of node `j`.
    dist: Vec<f64>,
    /// `dist_rev[i * n + j]` = distance from exit of node `i` to reversed-entry
    /// (i.e., exit point) of node `j`.
    dist_rev: Vec<f64>,
}

impl DistanceMatrix {
    /// Builds distance matrices from a slice of TSP nodes.
    fn new(nodes: &[TspNode]) -> Self {
        let n = nodes.len();
        let mut dist = vec![0.0; n * n];
        let mut dist_rev = vec![0.0; n * n];

        for i in 0..n {
            for j in 0..n {
                // Exit of i to entry of j (normal).
                let dx = nodes[i].exit.x - nodes[j].entry.x;
                let dy = nodes[i].exit.y - nodes[j].entry.y;
                dist[i * n + j] = (dx * dx + dy * dy).sqrt();

                // Exit of i to reversed-entry of j (i.e., exit of j used as entry).
                let dx_r = nodes[i].exit.x - nodes[j].exit.x;
                let dy_r = nodes[i].exit.y - nodes[j].exit.y;
                dist_rev[i * n + j] = (dx_r * dx_r + dy_r * dy_r).sqrt();
            }
        }

        Self { n, dist, dist_rev }
    }

    /// Distance from exit of node `from` to entry of node `to`.
    #[inline]
    fn dist(&self, from: usize, to: usize) -> f64 {
        self.dist[from * self.n + to]
    }

    /// Distance from exit of node `from` to reversed-entry of node `to`.
    #[inline]
    fn dist_reversed_entry(&self, from: usize, to: usize) -> f64 {
        self.dist_rev[from * self.n + to]
    }
}

/// Nearest-neighbor TSP construction heuristic.
///
/// Starting from `start_pos`, greedily selects the closest unvisited node
/// at each step. For reversible nodes, considers both orientations.
fn nearest_neighbor(nodes: &[TspNode], start_pos: Point2) -> Tour {
    let n = nodes.len();
    if n == 0 {
        return Tour::new(vec![]);
    }

    let mut visited = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut reversed = Vec::with_capacity(n);
    let mut current_pos = start_pos;

    for _ in 0..n {
        let mut best_dist = f64::INFINITY;
        let mut best_idx = 0;
        let mut best_reversed = false;

        for j in 0..n {
            if visited[j] {
                continue;
            }

            // Distance to normal entry.
            let dx = current_pos.x - nodes[j].entry.x;
            let dy = current_pos.y - nodes[j].entry.y;
            let d_normal = (dx * dx + dy * dy).sqrt();

            if d_normal < best_dist {
                best_dist = d_normal;
                best_idx = j;
                best_reversed = false;
            }

            // Distance to reversed entry (exit point used as entry) for reversible nodes.
            if nodes[j].reversible {
                let dx_r = current_pos.x - nodes[j].exit.x;
                let dy_r = current_pos.y - nodes[j].exit.y;
                let d_rev = (dx_r * dx_r + dy_r * dy_r).sqrt();

                if d_rev < best_dist {
                    best_dist = d_rev;
                    best_idx = j;
                    best_reversed = true;
                }
            }
        }

        visited[best_idx] = true;
        order.push(best_idx);
        reversed.push(best_reversed);

        // Update current position to exit of the chosen node.
        current_pos = if best_reversed && nodes[best_idx].reversible {
            nodes[best_idx].entry // reversed: entry becomes exit
        } else {
            nodes[best_idx].exit
        };
    }

    Tour { order, reversed }
}

/// Union-Find (Disjoint Set Union) data structure for cycle detection.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]]; // path compression
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, x: usize, y: usize) -> bool {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry {
            return false;
        }
        match self.rank[rx].cmp(&self.rank[ry]) {
            std::cmp::Ordering::Less => self.parent[rx] = ry,
            std::cmp::Ordering::Greater => self.parent[ry] = rx,
            std::cmp::Ordering::Equal => {
                self.parent[ry] = rx;
                self.rank[rx] += 1;
            }
        }
        true
    }

    fn connected(&mut self, x: usize, y: usize) -> bool {
        self.find(x) == self.find(y)
    }
}

/// Candidate edge for greedy edge insertion.
#[derive(Debug)]
struct CandidateEdge {
    from: usize,
    to: usize,
    distance: f64,
    to_reversed: bool,
}

/// Greedy edge insertion TSP construction heuristic.
///
/// Generates all candidate edges sorted by distance, then greedily adds
/// edges that do not violate degree constraints or create premature cycles.
#[allow(clippy::needless_range_loop)] // Indices needed for both matrix lookup and node access
fn greedy_edge_insertion(matrix: &DistanceMatrix, nodes: &[TspNode]) -> Tour {
    let n = nodes.len();
    if n == 0 {
        return Tour::new(vec![]);
    }
    if n == 1 {
        return Tour::new(vec![0]);
    }

    // Generate candidate edges.
    let mut edges: Vec<CandidateEdge> = Vec::with_capacity(n * n * 2);
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            // Normal orientation of j.
            edges.push(CandidateEdge {
                from: i,
                to: j,
                distance: matrix.dist(i, j),
                to_reversed: false,
            });

            // Reversed orientation of j (if reversible).
            if nodes[j].reversible {
                edges.push(CandidateEdge {
                    from: i,
                    to: j,
                    distance: matrix.dist_reversed_entry(i, j),
                    to_reversed: true,
                });
            }
        }
    }
    edges.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

    let mut uf = UnionFind::new(n);
    let mut out_degree = vec![0usize; n];
    let mut in_degree = vec![0usize; n];
    let mut next: Vec<Option<usize>> = vec![None; n];
    let mut prev: Vec<Option<usize>> = vec![None; n];
    let mut node_reversed = vec![false; n];
    let mut edge_count = 0;

    for edge in &edges {
        if edge_count >= n - 1 {
            break;
        }

        let from = edge.from;
        let to = edge.to;

        // Check degree constraints: out_degree(from) < 1, in_degree(to) < 1.
        if out_degree[from] >= 1 || in_degree[to] >= 1 {
            continue;
        }

        // Check cycle: would connecting from->to create a cycle before all nodes joined?
        if edge_count < n - 1 && uf.connected(from, to) {
            continue;
        }

        // Accept edge.
        next[from] = Some(to);
        prev[to] = Some(from);
        out_degree[from] += 1;
        in_degree[to] += 1;
        uf.union(from, to);
        edge_count += 1;

        // Track reversal state for the target node.
        if edge.to_reversed {
            node_reversed[to] = true;
        }
    }

    // Build tour by following the chain from a node with no predecessor.
    let start = (0..n).find(|&i| prev[i].is_none()).unwrap_or(0);
    let mut order = Vec::with_capacity(n);
    let mut reversed_flags = Vec::with_capacity(n);
    let mut current = start;
    let mut visited = vec![false; n];

    loop {
        if visited[current] {
            break;
        }
        visited[current] = true;
        order.push(current);
        reversed_flags.push(node_reversed[current]);

        match next[current] {
            Some(nxt) if !visited[nxt] => current = nxt,
            _ => break,
        }
    }

    // Add any unvisited nodes (disconnected fragments).
    for i in 0..n {
        if !visited[i] {
            order.push(i);
            reversed_flags.push(node_reversed[i]);
        }
    }

    Tour {
        order,
        reversed: reversed_flags,
    }
}

/// 2-opt local search improvement for a TSP tour.
///
/// Iteratively reverses sub-segments of the tour when doing so reduces total
/// travel distance. Stops when no improvement is found or `max_iterations`
/// passes are exhausted (0 = no limit).
fn two_opt_improve(
    tour: &mut Tour,
    nodes: &[TspNode],
    max_iterations: u32,
) {
    let n = tour.order.len();
    if n <= 2 {
        return;
    }

    let mut iteration = 0;
    loop {
        if max_iterations > 0 && iteration >= max_iterations {
            break;
        }
        iteration += 1;

        let mut improved = false;

        for i in 0..n - 1 {
            for j in (i + 2)..n {
                // Skip if j wraps around to be adjacent to i.
                if i == 0 && j == n - 1 {
                    continue;
                }

                // Compute delta: cost of removing edges (i, i+1) and (j, j+1 or end)
                // and adding edges (i, j) and (i+1, j+1 or end) with reversed segment.

                // Current edges cost.
                let d_i_ip1 = tour.edge_distance(nodes, i, i + 1);
                let d_j_jp1 = if j + 1 < n {
                    tour.edge_distance(nodes, j, j + 1)
                } else {
                    0.0
                };

                // Compute cost with reversed segment [i+1..=j].
                // After reversal: edge from i to j, then from i+1 to j+1.
                // But we need to account for flipped directions of reversible nodes.

                // New edge: from tour[i] to what was tour[j] (now reversed if reversible).
                let from_node = tour.order[i];
                let from_rev = tour.reversed[i];
                let to_node = tour.order[j];
                // After reversal, the node at position i+1 is the old node at j, but
                // its direction is flipped if it's reversible.
                let to_rev_flipped = if nodes[to_node].reversible {
                    !tour.reversed[j]
                } else {
                    tour.reversed[j]
                };

                let from_exit = if from_rev && nodes[from_node].reversible {
                    &nodes[from_node].entry
                } else {
                    &nodes[from_node].exit
                };
                let to_entry = if to_rev_flipped && nodes[to_node].reversible {
                    &nodes[to_node].exit
                } else {
                    &nodes[to_node].entry
                };
                let dx1 = from_exit.x - to_entry.x;
                let dy1 = from_exit.y - to_entry.y;
                let new_d_i_ip1 = (dx1 * dx1 + dy1 * dy1).sqrt();

                // New edge: from what was tour[i+1] (now at position j, reversed) to tour[j+1].
                let new_d_j_jp1 = if j + 1 < n {
                    let from2_node = tour.order[i + 1];
                    let from2_rev_flipped = if nodes[from2_node].reversible {
                        !tour.reversed[i + 1]
                    } else {
                        tour.reversed[i + 1]
                    };
                    let to2_node = tour.order[j + 1];
                    let to2_rev = tour.reversed[j + 1];

                    let from2_exit = if from2_rev_flipped && nodes[from2_node].reversible {
                        &nodes[from2_node].entry
                    } else {
                        &nodes[from2_node].exit
                    };
                    let to2_entry = if to2_rev && nodes[to2_node].reversible {
                        &nodes[to2_node].exit
                    } else {
                        &nodes[to2_node].entry
                    };
                    let dx2 = from2_exit.x - to2_entry.x;
                    let dy2 = from2_exit.y - to2_entry.y;
                    (dx2 * dx2 + dy2 * dy2).sqrt()
                } else {
                    0.0
                };

                let delta = (new_d_i_ip1 + new_d_j_jp1) - (d_i_ip1 + d_j_jp1);

                if delta < -1e-9 {
                    // Apply the reversal.
                    // Reverse order of indices in [i+1..=j].
                    tour.order[i + 1..=j].reverse();
                    tour.reversed[i + 1..=j].reverse();

                    // Flip reversed flag for reversible nodes in the segment.
                    for k in (i + 1)..=j {
                        if nodes[tour.order[k]].reversible {
                            tour.reversed[k] = !tour.reversed[k];
                        }
                    }

                    improved = true;
                }
            }
        }

        if !improved {
            break;
        }
    }
}

/// Optimizes the order of TSP nodes to minimize travel distance.
///
/// Returns a vector of `(original_index, reversed)` tuples representing the
/// optimized print order. The `reversed` flag indicates whether reversible
/// paths should be printed in reverse direction.
///
/// # Arguments
///
/// * `nodes` - The printable elements to reorder.
/// * `start_pos` - The nozzle position before this layer.
/// * `config` - Travel optimization configuration.
///
/// # Short-circuits
///
/// * 0 nodes: returns empty vector
/// * 1 node: returns `[(original_index, false)]`
/// * 2 nodes: compares both orderings and returns shorter
pub fn optimize_tour(
    nodes: &[TspNode],
    start_pos: Point2,
    config: &TravelOptConfig,
) -> Vec<(usize, bool)> {
    let n = nodes.len();

    // Short-circuit: empty.
    if n == 0 {
        return vec![];
    }

    // Short-circuit: single node.
    if n == 1 {
        return vec![(nodes[0].original_index, false)];
    }

    // Short-circuit: two nodes -- compare both orderings.
    if n == 2 {
        return optimize_two_nodes(nodes, start_pos);
    }

    let matrix = DistanceMatrix::new(nodes);

    match config.algorithm {
        TravelOptAlgorithm::Auto => {
            let nn_tour = nearest_neighbor(nodes, start_pos);
            let greedy_tour = greedy_edge_insertion(&matrix, nodes);

            let nn_dist = nn_tour.total_distance(nodes);
            let greedy_dist = greedy_tour.total_distance(nodes);

            if n <= 30 {
                // For small problems, 2-opt both and pick the best.
                let mut nn_opt = nn_tour;
                two_opt_improve(&mut nn_opt, nodes, config.max_iterations);
                let mut greedy_opt = greedy_tour;
                two_opt_improve(&mut greedy_opt, nodes, config.max_iterations);

                let nn_opt_dist = nn_opt.total_distance(nodes);
                let greedy_opt_dist = greedy_opt.total_distance(nodes);

                if nn_opt_dist <= greedy_opt_dist {
                    nn_opt.to_permutation(nodes)
                } else {
                    greedy_opt.to_permutation(nodes)
                }
            } else {
                // For larger problems, 2-opt only the shorter initial tour.
                let mut best = if nn_dist <= greedy_dist {
                    nn_tour
                } else {
                    greedy_tour
                };
                two_opt_improve(&mut best, nodes, config.max_iterations);
                best.to_permutation(nodes)
            }
        }
        TravelOptAlgorithm::NearestNeighbor => {
            let mut tour = nearest_neighbor(nodes, start_pos);
            two_opt_improve(&mut tour, nodes, config.max_iterations);
            tour.to_permutation(nodes)
        }
        TravelOptAlgorithm::GreedyEdgeInsertion => {
            let mut tour = greedy_edge_insertion(&matrix, nodes);
            two_opt_improve(&mut tour, nodes, config.max_iterations);
            tour.to_permutation(nodes)
        }
        TravelOptAlgorithm::NearestNeighborOnly => {
            let tour = nearest_neighbor(nodes, start_pos);
            tour.to_permutation(nodes)
        }
        TravelOptAlgorithm::GreedyOnly => {
            let tour = greedy_edge_insertion(&matrix, nodes);
            tour.to_permutation(nodes)
        }
        // non_exhaustive: future variants fall back to Auto behavior.
        #[allow(unreachable_patterns)]
        _ => {
            let mut tour = nearest_neighbor(nodes, start_pos);
            two_opt_improve(&mut tour, nodes, config.max_iterations);
            tour.to_permutation(nodes)
        }
    }
}

/// Optimizes ordering of exactly two nodes by comparing both orderings.
fn optimize_two_nodes(nodes: &[TspNode], start_pos: Point2) -> Vec<(usize, bool)> {
    // Consider all 4 combinations for 2 nodes (normal/reversed for each, in both orders).
    let mut best_dist = f64::INFINITY;
    let mut best = vec![(nodes[0].original_index, false), (nodes[1].original_index, false)];

    for &(first, second) in &[(0usize, 1usize), (1, 0)] {
        for &first_rev in &[false, true] {
            if first_rev && !nodes[first].reversible {
                continue;
            }
            for &second_rev in &[false, true] {
                if second_rev && !nodes[second].reversible {
                    continue;
                }

                let first_entry = if first_rev {
                    &nodes[first].exit
                } else {
                    &nodes[first].entry
                };
                let first_exit = if first_rev {
                    &nodes[first].entry
                } else {
                    &nodes[first].exit
                };
                let second_entry = if second_rev {
                    &nodes[second].exit
                } else {
                    &nodes[second].entry
                };

                let dx1 = start_pos.x - first_entry.x;
                let dy1 = start_pos.y - first_entry.y;
                let d1 = (dx1 * dx1 + dy1 * dy1).sqrt();

                let dx2 = first_exit.x - second_entry.x;
                let dy2 = first_exit.y - second_entry.y;
                let d2 = (dx2 * dx2 + dy2 * dy2).sqrt();

                let total = d1 + d2;
                if total < best_dist {
                    best_dist = total;
                    best = vec![
                        (nodes[first].original_index, first_rev),
                        (nodes[second].original_index, second_rev),
                    ];
                }
            }
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(entry: (f64, f64), exit: (f64, f64), idx: usize) -> TspNode {
        TspNode {
            entry: Point2::new(entry.0, entry.1),
            exit: Point2::new(exit.0, exit.1),
            reversible: false,
            original_index: idx,
        }
    }

    fn make_reversible_node(entry: (f64, f64), exit: (f64, f64), idx: usize) -> TspNode {
        TspNode {
            entry: Point2::new(entry.0, entry.1),
            exit: Point2::new(exit.0, exit.1),
            reversible: true,
            original_index: idx,
        }
    }

    fn default_config() -> TravelOptConfig {
        TravelOptConfig::default()
    }

    #[test]
    fn test_empty_nodes() {
        let result = optimize_tour(&[], Point2::zero(), &default_config());
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_node() {
        let nodes = vec![make_node((1.0, 1.0), (1.0, 1.0), 0)];
        let result = optimize_tour(&nodes, Point2::zero(), &default_config());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 0);
    }

    #[test]
    fn test_two_nodes() {
        // Node 0 at (1,0), Node 1 at (10,0). Starting from origin.
        // Optimal: visit 0 first (closer), then 1.
        let nodes = vec![
            make_node((1.0, 0.0), (1.0, 0.0), 0),
            make_node((10.0, 0.0), (10.0, 0.0), 1),
        ];
        let result = optimize_tour(&nodes, Point2::zero(), &default_config());
        assert_eq!(result.len(), 2);
        // Both original indices should be present.
        let indices: Vec<usize> = result.iter().map(|(idx, _)| *idx).collect();
        assert!(indices.contains(&0));
        assert!(indices.contains(&1));
        // Node 0 should come first (closer to origin).
        assert_eq!(result[0].0, 0);
    }

    #[test]
    fn test_nn_four_points() {
        // Square: (0,0), (1,0), (1,1), (0,1). Start at origin.
        // NN from origin should visit in a reasonable order (all nodes visited).
        let nodes = vec![
            make_node((0.0, 0.0), (0.0, 0.0), 0),
            make_node((1.0, 0.0), (1.0, 0.0), 1),
            make_node((1.0, 1.0), (1.0, 1.0), 2),
            make_node((0.0, 1.0), (0.0, 1.0), 3),
        ];
        let tour = nearest_neighbor(&nodes, Point2::zero());

        assert_eq!(tour.len(), 4);
        let mut indices: Vec<usize> = tour.order.clone();
        indices.sort_unstable();
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_greedy_four_points() {
        let nodes = vec![
            make_node((0.0, 0.0), (0.0, 0.0), 0),
            make_node((1.0, 0.0), (1.0, 0.0), 1),
            make_node((1.0, 1.0), (1.0, 1.0), 2),
            make_node((0.0, 1.0), (0.0, 1.0), 3),
        ];
        let matrix = DistanceMatrix::new(&nodes);
        let tour = greedy_edge_insertion(&matrix, &nodes);

        assert_eq!(tour.len(), 4);
        let mut indices: Vec<usize> = tour.order.clone();
        indices.sort_unstable();
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_two_opt_improves() {
        // Create a deliberately crossing tour: visiting A(0,0)->C(1,1)->B(1,0)->D(0,1)
        // which crosses. 2-opt should uncross to A->B->C->D or A->D->C->B.
        let nodes = vec![
            make_node((0.0, 0.0), (0.0, 0.0), 0), // A
            make_node((1.0, 1.0), (1.0, 1.0), 1), // C
            make_node((1.0, 0.0), (1.0, 0.0), 2), // B
            make_node((0.0, 1.0), (0.0, 1.0), 3), // D
        ];
        // Crossing tour: A -> C -> B -> D.
        let mut tour = Tour {
            order: vec![0, 1, 2, 3],
            reversed: vec![false; 4],
        };

        let before = tour.total_distance(&nodes);
        two_opt_improve(&mut tour, &nodes, 100);
        let after = tour.total_distance(&nodes);

        assert!(
            after <= before + 1e-9,
            "2-opt should not increase tour length: before={before}, after={after}"
        );
        // The crossing tour has length sqrt(2) + sqrt(2) + sqrt(2) = 3*sqrt(2) ~ 4.24
        // Optimal non-crossing is 1 + sqrt(2) + 1 = 2 + sqrt(2) ~ 3.41
        assert!(
            after < before - 0.1,
            "2-opt should significantly improve crossing tour: before={before}, after={after}"
        );
    }

    #[test]
    fn test_auto_picks_best() {
        // 6 points in a line; Auto should produce a tour no worse than either individual algorithm.
        let nodes: Vec<TspNode> = (0..6)
            .map(|i| {
                let x = f64::from(i) * 10.0;
                make_node((x, 0.0), (x, 0.0), i as usize)
            })
            .collect();

        let auto_config = TravelOptConfig {
            algorithm: TravelOptAlgorithm::Auto,
            ..TravelOptConfig::default()
        };
        let nn_config = TravelOptConfig {
            algorithm: TravelOptAlgorithm::NearestNeighborOnly,
            ..TravelOptConfig::default()
        };
        let greedy_config = TravelOptConfig {
            algorithm: TravelOptAlgorithm::GreedyOnly,
            ..TravelOptConfig::default()
        };

        let start = Point2::zero();

        let auto_result = optimize_tour(&nodes, start, &auto_config);
        let nn_result = optimize_tour(&nodes, start, &nn_config);
        let greedy_result = optimize_tour(&nodes, start, &greedy_config);

        // Compute total distances for each result.
        let auto_dist = tour_distance_from_result(&auto_result, &nodes, start);
        let nn_dist = tour_distance_from_result(&nn_result, &nodes, start);
        let greedy_dist = tour_distance_from_result(&greedy_result, &nodes, start);

        let best_simple = nn_dist.min(greedy_dist);
        assert!(
            auto_dist <= best_simple + 1e-6,
            "Auto ({auto_dist}) should be no worse than best of NN ({nn_dist}) and greedy ({greedy_dist})"
        );
    }

    #[test]
    fn test_reversible_nodes() {
        // Open path from (0,0) to (10,0). If we start at (10,0), reversing makes
        // entry at (10,0) which is closer.
        let nodes = vec![make_reversible_node((0.0, 0.0), (10.0, 0.0), 0)];
        let result = optimize_tour(&nodes, Point2::new(10.0, 0.0), &default_config());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 0);
        // Single node is never reversed in the single-node short-circuit.
    }

    #[test]
    fn test_reversible_two_nodes() {
        // Two reversible paths. Starting near exit of node 0.
        // Node 0: entry=(0,0), exit=(5,0)
        // Node 1: entry=(6,0), exit=(10,0)
        // Starting at (5,0) -- should visit node 0 reversed (entry at exit=5,0) then node 1.
        let nodes = vec![
            make_reversible_node((0.0, 0.0), (5.0, 0.0), 0),
            make_reversible_node((6.0, 0.0), (10.0, 0.0), 1),
        ];
        let result = optimize_tour(&nodes, Point2::new(5.0, 0.0), &default_config());
        assert_eq!(result.len(), 2);
        // Both indices present.
        let indices: Vec<usize> = result.iter().map(|(idx, _)| *idx).collect();
        assert!(indices.contains(&0));
        assert!(indices.contains(&1));
    }

    #[test]
    fn test_distance_matrix_asymmetric() {
        // Node with different entry and exit -> dist(i,j) != dist(j,i).
        let nodes = vec![
            make_node((0.0, 0.0), (5.0, 0.0), 0), // exit at (5,0)
            make_node((3.0, 0.0), (3.0, 0.0), 1),  // entry/exit at (3,0)
        ];
        let matrix = DistanceMatrix::new(&nodes);

        let d01 = matrix.dist(0, 1); // exit(0)=(5,0) to entry(1)=(3,0) = 2
        let d10 = matrix.dist(1, 0); // exit(1)=(3,0) to entry(0)=(0,0) = 3

        assert!(
            (d01 - 2.0).abs() < 1e-9,
            "Expected d(0->1)=2, got {d01}"
        );
        assert!(
            (d10 - 3.0).abs() < 1e-9,
            "Expected d(1->0)=3, got {d10}"
        );
        assert!(
            (d01 - d10).abs() > 0.5,
            "Distance matrix should be asymmetric"
        );
    }

    /// Helper: compute total travel distance from optimize_tour result.
    fn tour_distance_from_result(
        result: &[(usize, bool)],
        nodes: &[TspNode],
        start: Point2,
    ) -> f64 {
        if result.is_empty() {
            return 0.0;
        }
        let mut total = 0.0;
        let mut pos = start;

        for &(orig_idx, reversed) in result {
            let node = nodes.iter().find(|n| n.original_index == orig_idx).unwrap();
            let entry = if reversed { &node.exit } else { &node.entry };
            let exit = if reversed { &node.entry } else { &node.exit };

            let dx = pos.x - entry.x;
            let dy = pos.y - entry.y;
            total += (dx * dx + dy * dy).sqrt();
            pos = *exit;
        }
        total
    }
}
