//! Pathfinding over hex graphs: A*, budgeted Dijkstra ("where can I move this turn?") and flow
//! fields (many units heading to shared goals).
//!
//! All searches take a step-cost callback `cost(from, to) -> Option<u32>`; `None` means the step
//! is blocked. That single hook covers terrain costs, climb limits, occupied hexes, map bounds and
//! zones of control.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::{Hex, HexMap};

/// A found path including both endpoints, with its total cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub hexes: Vec<Hex>,
    pub cost: u32,
}

/// A* from `start` to `goal`. `min_step_cost` scales the distance heuristic; pass the cheapest
/// possible step cost (1 if unsure) to keep the heuristic admissible. `max_cost` bounds the
/// search (use `u32::MAX` for unbounded).
pub fn astar(
    start: Hex,
    goal: Hex,
    min_step_cost: u32,
    max_cost: u32,
    mut cost: impl FnMut(Hex, Hex) -> Option<u32>,
) -> Option<Path> {
    if start == goal {
        return Some(Path { hexes: vec![start], cost: 0 });
    }
    let h = |x: Hex| x.distance(goal) as u32 * min_step_cost;
    let mut open = BinaryHeap::new();
    let mut came: HexMap<(Hex, u32)> = HexMap::new();
    came.insert(start, (start, 0));
    // Tie-break on a counter so equal-f nodes expand in insertion order (straighter paths).
    let mut counter = 0u32;
    open.push(Reverse((h(start), 0u32, counter, start)));
    while let Some(Reverse((_, g, _, cur))) = open.pop() {
        if cur == goal {
            return Some(Path { hexes: reconstruct(&came, start, goal), cost: g });
        }
        if came.get(cur).is_some_and(|&(_, best)| best < g) {
            continue;
        }
        for n in cur.neighbors() {
            let Some(step) = cost(cur, n) else { continue };
            let ng = g.saturating_add(step);
            if ng > max_cost {
                continue;
            }
            if came.get(n).is_none_or(|&(_, best)| ng < best) {
                came.insert(n, (cur, ng));
                counter += 1;
                open.push(Reverse((ng + h(n), ng, counter, n)));
            }
        }
    }
    None
}

fn reconstruct(came: &HexMap<(Hex, u32)>, start: Hex, goal: Hex) -> Vec<Hex> {
    let mut out = vec![goal];
    let mut cur = goal;
    while cur != start {
        cur = came[cur].0;
        out.push(cur);
    }
    out.reverse();
    out
}

/// Result of a budgeted Dijkstra search: every hex reachable within the budget.
#[derive(Clone, Debug, Default)]
pub struct Reachable {
    pub start: Hex,
    /// hex → (previous hex, total cost)
    pub nodes: HexMap<(Hex, u32)>,
}

impl Reachable {
    pub fn contains(&self, h: Hex) -> bool {
        self.nodes.contains(h)
    }

    pub fn cost(&self, h: Hex) -> Option<u32> {
        self.nodes.get(h).map(|&(_, c)| c)
    }

    pub fn path_to(&self, h: Hex) -> Option<Vec<Hex>> {
        self.nodes.contains(h).then(|| reconstruct(&self.nodes, self.start, h))
    }

    pub fn hexes(&self) -> impl Iterator<Item = Hex> + '_ {
        self.nodes.hexes()
    }
}

/// All hexes reachable from `start` with total cost `<= budget`.
pub fn reachable(start: Hex, budget: u32, mut cost: impl FnMut(Hex, Hex) -> Option<u32>) -> Reachable {
    let mut nodes: HexMap<(Hex, u32)> = HexMap::new();
    nodes.insert(start, (start, 0));
    let mut open = BinaryHeap::new();
    open.push(Reverse((0u32, start)));
    while let Some(Reverse((g, cur))) = open.pop() {
        if nodes.get(cur).is_some_and(|&(_, best)| best < g) {
            continue;
        }
        for n in cur.neighbors() {
            let Some(step) = cost(cur, n) else { continue };
            let ng = g.saturating_add(step);
            if ng > budget {
                continue;
            }
            if nodes.get(n).is_none_or(|&(_, best)| ng < best) {
                nodes.insert(n, (cur, ng));
                open.push(Reverse((ng, n)));
            }
        }
    }
    Reachable { start, nodes }
}

/// Unweighted breadth-first distances from `start`, limited to `max_steps`.
pub fn bfs(start: Hex, max_steps: u32, mut passable: impl FnMut(Hex) -> bool) -> HexMap<u32> {
    let mut dist = HexMap::new();
    dist.insert(start, 0);
    let mut queue = VecDeque::from([start]);
    while let Some(cur) = queue.pop_front() {
        let d = dist[cur];
        if d >= max_steps {
            continue;
        }
        for n in cur.neighbors() {
            if !dist.contains(n) && passable(n) {
                dist.insert(n, d + 1);
                queue.push_back(n);
            }
        }
    }
    dist
}

/// A flow field: for every hex in the searched area, the cost to the nearest goal and the next
/// hex to step to. Build once per goal set, then any number of units can follow it cheaply.
#[derive(Clone, Debug, Default)]
pub struct FlowField {
    /// hex → (next hex towards a goal, remaining cost)
    pub nodes: HexMap<(Hex, u32)>,
}

impl FlowField {
    /// Dijkstra outwards from all `goals`. `cost(from, to)` is the cost of a unit stepping from
    /// `from` to `to` (it is evaluated in the unit's direction of travel). `max_cost` bounds the
    /// search area.
    pub fn build(
        goals: impl IntoIterator<Item = Hex>,
        max_cost: u32,
        mut cost: impl FnMut(Hex, Hex) -> Option<u32>,
    ) -> FlowField {
        let mut nodes: HexMap<(Hex, u32)> = HexMap::new();
        let mut open = BinaryHeap::new();
        for g in goals {
            nodes.insert(g, (g, 0));
            open.push(Reverse((0u32, g)));
        }
        while let Some(Reverse((d, cur))) = open.pop() {
            if nodes.get(cur).is_some_and(|&(_, best)| best < d) {
                continue;
            }
            for n in cur.neighbors() {
                // A unit at `n` would step to `cur`.
                let Some(step) = cost(n, cur) else { continue };
                let nd = d.saturating_add(step);
                if nd > max_cost {
                    continue;
                }
                if nodes.get(n).is_none_or(|&(_, best)| nd < best) {
                    nodes.insert(n, (cur, nd));
                    open.push(Reverse((nd, n)));
                }
            }
        }
        FlowField { nodes }
    }

    /// The next hex to step to from `h`, or `None` if `h` is outside the field. Goals return
    /// themselves.
    pub fn next(&self, h: Hex) -> Option<Hex> {
        self.nodes.get(h).map(|&(n, _)| n)
    }

    pub fn cost(&self, h: Hex) -> Option<u32> {
        self.nodes.get(h).map(|&(_, c)| c)
    }

    pub fn is_goal(&self, h: Hex) -> bool {
        self.nodes.get(h).is_some_and(|&(n, _)| n == h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes;

    fn open_map(radius: i32) -> impl FnMut(Hex, Hex) -> Option<u32> {
        move |_, to: Hex| (to.length() <= radius).then_some(1)
    }

    #[test]
    fn astar_straight_line() {
        let p = astar(Hex::ORIGIN, Hex::new(4, -2), 1, u32::MAX, open_map(10)).unwrap();
        assert_eq!(p.cost, 4);
        assert_eq!(p.hexes.len(), 5);
        for w in p.hexes.windows(2) {
            assert_eq!(w[0].distance(w[1]), 1);
        }
    }

    #[test]
    fn astar_around_wall() {
        // A wall along q = 1 except far away.
        let wall = |h: Hex| h.q == 1 && h.r > -4 && h.r < 4;
        let p = astar(Hex::ORIGIN, Hex::new(2, 0), 1, u32::MAX, |_, to| (to.length() <= 8 && !wall(to)).then_some(1))
            .unwrap();
        assert!(p.cost > 2);
        assert!(p.hexes.iter().all(|&h| !wall(h)));
        assert!(astar(Hex::ORIGIN, Hex::new(2, 0), 1, 3, |_, to| (!wall(to)).then_some(1)).is_none());
    }

    #[test]
    fn astar_prefers_cheap_terrain() {
        let swamp = |h: Hex| h.r == 0 && h.q > 0 && h.q < 4;
        let p = astar(Hex::ORIGIN, Hex::new(4, 0), 1, u32::MAX, |_, to| Some(if swamp(to) { 10 } else { 1 })).unwrap();
        assert!(p.hexes.iter().all(|&h| !swamp(h)));
    }

    #[test]
    fn reachable_budget() {
        let r = reachable(Hex::ORIGIN, 2, open_map(10));
        assert_eq!(r.nodes.len(), 19);
        assert_eq!(r.cost(Hex::new(2, 0)), Some(2));
        assert_eq!(r.path_to(Hex::new(2, 0)).unwrap().len(), 3);
        assert!(!r.contains(Hex::new(3, 0)));
    }

    #[test]
    fn bfs_distances() {
        let d = bfs(Hex::ORIGIN, 3, |h| h != Hex::new(1, 0));
        assert_eq!(d[Hex::new(-3, 0)], 3);
        assert!(!d.contains(Hex::new(1, 0)));
    }

    #[test]
    fn flow_field_leads_to_goal() {
        let area: crate::HexSet = shapes::hexagon(Hex::ORIGIN, 6).into_iter().collect();
        let ff = FlowField::build([Hex::new(3, -3)], u32::MAX, |_, to| area.contains(&to).then_some(1));
        for start in shapes::hexagon(Hex::ORIGIN, 6) {
            let mut h = start;
            let mut steps = 0;
            while !ff.is_goal(h) {
                h = ff.next(h).unwrap();
                steps += 1;
                assert!(steps < 50);
            }
            assert_eq!(steps, start.distance(Hex::new(3, -3)));
        }
    }
}
