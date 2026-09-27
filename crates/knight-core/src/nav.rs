//! Hex navigation: the one way things move in a Knight world.
//!
//! A [`HexMover`] is always either standing on a hex centre or stepping between two adjacent
//! hexes. Its position can only be changed by giving it a route of adjacent hexes (or a single
//! neighbour); there is no API for free movement. That keeps every game on the grid — turn based
//! or real time — and makes occupancy, reservations and pathfinding exact.
//!
//! [`Reservations`] adds real-time traffic rules: a mover must reserve the next hex before
//! stepping into it, so units never overlap and queue behind each other instead.

use std::collections::VecDeque;

use glam::Vec3;
use knight_hex::{Hex, HexMap};

use crate::world::HexWorld;

/// Why a route was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavError {
    /// The route does not start next to (or at) where the mover is heading.
    Disconnected,
    /// Hex `index` of the route (as given) is not a neighbour of the one before it.
    NotAdjacent { index: usize },
}

impl std::fmt::Display for NavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NavError::Disconnected => write!(f, "route does not start at the mover's hex"),
            NavError::NotAdjacent { index } => write!(f, "route step {index} is not between adjacent hexes"),
        }
    }
}

impl std::error::Error for NavError {}

/// What happened during [`HexMover::update`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Started stepping from the first hex into the second.
    Left(Hex, Hex),
    /// Finished stepping into this hex (it is now the mover's hex).
    Arrived(Hex),
    /// Wanted to step into this hex but was not allowed to (occupied, blocked).
    Blocked(Hex),
}

/// Moves one thing through the hex grid, one hex at a time.
#[derive(Clone, Debug)]
pub struct HexMover {
    /// Hex it stands on, or is leaving.
    from: Hex,
    /// Hex it is stepping into, if any.
    to: Option<Hex>,
    /// Progress of the current step, 0..1.
    t: f32,
    /// Hexes still to visit after `to`.
    route: VecDeque<Hex>,
    /// Hexes per second on flat, cost-1 ground.
    pub speed: f32,
    /// Seconds spent waiting for a blocked hex (reset when moving again).
    pub blocked_for: f32,
    /// Direction of the last step (for facing sprites): +1 right, -1 left, 0 unknown.
    pub heading_x: f32,
}

impl HexMover {
    pub fn new(hex: Hex, speed: f32) -> Self {
        HexMover { from: hex, to: None, t: 0.0, route: VecDeque::new(), speed, blocked_for: 0.0, heading_x: 0.0 }
    }

    /// The hex it stands on; while stepping, the one it is closer to.
    pub fn hex(&self) -> Hex {
        match self.to {
            Some(to) if self.t >= 0.5 => to,
            _ => self.from,
        }
    }

    /// Where it will stand once the current step ends (its own hex when idle).
    pub fn next_stop(&self) -> Hex {
        self.to.unwrap_or(self.from)
    }

    /// Final hex of the route.
    pub fn destination(&self) -> Hex {
        self.route.back().copied().unwrap_or(self.next_stop())
    }

    /// The hex being left and the hex being entered, while stepping.
    pub fn step(&self) -> Option<(Hex, Hex)> {
        self.to.map(|to| (self.from, to))
    }

    pub fn is_moving(&self) -> bool {
        self.to.is_some() || !self.route.is_empty()
    }

    pub fn route(&self) -> impl Iterator<Item = Hex> + '_ {
        self.to.into_iter().chain(self.route.iter().copied())
    }

    /// Follow `path`, a list of adjacent hexes. It may start with the hex the mover is heading
    /// to (as returned by pathfinding from [`HexMover::next_stop`]) or with a neighbour of it.
    /// A step in progress is always finished first.
    pub fn set_route(&mut self, path: &[Hex]) -> Result<(), NavError> {
        let start = self.next_stop();
        let (path, skipped) = match path.first() {
            Some(&h) if h == start => (&path[1..], 1),
            Some(&h) if h.distance(start) == 1 => (path, 0),
            Some(_) => return Err(NavError::Disconnected),
            None => (&[][..], 0),
        };
        for (i, w) in path.windows(2).enumerate() {
            if w[0].distance(w[1]) != 1 {
                // Index into the route as given.
                return Err(NavError::NotAdjacent { index: i + 1 + skipped });
            }
        }
        self.route = path.iter().copied().collect();
        Ok(())
    }

    /// Step into one neighbouring hex (of where it is heading).
    pub fn step_to(&mut self, neighbour: Hex) -> Result<(), NavError> {
        self.set_route(&[neighbour])
    }

    /// Stop at the end of the current step (it never stops between hexes).
    pub fn stop(&mut self) {
        self.route.clear();
    }

    /// Teleport onto a hex (spawning, respawning). Clears the route.
    pub fn place(&mut self, hex: Hex) {
        self.from = hex;
        self.to = None;
        self.t = 0.0;
        self.route.clear();
    }

    /// Advance. `step_time(from, to)` returns the time multiplier for a step (terrain cost:
    /// 1 = normal, 2 = twice as slow) or `None` if the step is not allowed right now — then the
    /// mover waits and reports [`Step::Blocked`] (it is asked again next update).
    pub fn update(&mut self, dt: f32, mut step_time: impl FnMut(Hex, Hex) -> Option<f32>) -> Vec<Step> {
        let mut events = Vec::new();
        let mut left = dt;
        // Loop so fast movers can cross several hexes in one long frame.
        for _ in 0..8 {
            if self.to.is_none() {
                let Some(&next) = self.route.front() else { break };
                match step_time(self.from, next) {
                    Some(_) => {
                        self.route.pop_front();
                        self.to = Some(next);
                        self.t = 0.0;
                        self.blocked_for = 0.0;
                        events.push(Step::Left(self.from, next));
                    }
                    None => {
                        self.blocked_for += left;
                        events.push(Step::Blocked(next));
                        break;
                    }
                }
            }
            let Some(to) = self.to else { break };
            let mult = step_time(self.from, to).unwrap_or(1.0).max(0.05);
            let rate = self.speed / mult;
            let need = (1.0 - self.t) / rate.max(1e-6);
            if left < need {
                self.t += left * rate;
                break;
            }
            left -= need;
            self.from = to;
            self.to = None;
            self.t = 0.0;
            events.push(Step::Arrived(to));
        }
        events
    }

    /// World position: hex centres interpolated, following the terrain with a small hop when
    /// changing height level.
    pub fn position(&self, world: &HexWorld) -> Vec3 {
        let a = world.hex_to_world(self.from);
        let Some(to) = self.to else { return a };
        let b = world.hex_to_world(to);
        let t = self.t;
        let mut p = a.lerp(b, t);
        if (a.y - b.y).abs() > 1e-3 {
            // Stay on the higher ground until the edge, then hop.
            let s = t * t * (3.0 - 2.0 * t);
            p.y = a.y + (b.y - a.y) * s + (t * (1.0 - t)) * world.height_step * 1.5;
        }
        p
    }

    /// Update `heading_x` from the current step, for sprite facing.
    pub fn heading(&mut self, world: &HexWorld) -> f32 {
        if let Some(to) = self.to {
            let dx = world.hex_to_world(to).x - world.hex_to_world(self.from).x;
            if dx.abs() > 1e-3 {
                self.heading_x = dx.signum();
            }
        }
        self.heading_x
    }
}

/// Who holds which hex, for real-time movement without overlap. A mover holds the hex it stands
/// on and, while stepping, the hex it steps into.
#[derive(Clone, Debug, Default)]
pub struct Reservations {
    owner: HexMap<u32>,
    held: std::collections::HashMap<u32, Vec<Hex>>,
}

impl Reservations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn owner(&self, h: Hex) -> Option<u32> {
        self.owner.get(h).copied()
    }

    pub fn is_free(&self, h: Hex, for_id: u32) -> bool {
        self.owner.get(h).is_none_or(|&o| o == for_id)
    }

    /// Take `h` for `id` if free (or already ours).
    pub fn reserve(&mut self, h: Hex, id: u32) -> bool {
        match self.owner.get(h) {
            Some(&o) => o == id,
            None => {
                self.owner.insert(h, id);
                self.held.entry(id).or_default().push(h);
                true
            }
        }
    }

    pub fn release(&mut self, h: Hex, id: u32) {
        if self.owner.get(h) == Some(&id) {
            self.owner.remove(h);
            if let Some(v) = self.held.get_mut(&id) {
                v.retain(|&x| x != h);
            }
        }
    }

    /// Release everything `id` holds (it died or left).
    pub fn release_all(&mut self, id: u32) {
        for h in self.held.remove(&id).unwrap_or_default() {
            self.owner.remove(h);
        }
    }

    /// Hexes held by `id`.
    pub fn held_by(&self, id: u32) -> &[Hex] {
        self.held.get(&id).map_or(&[], |v| v.as_slice())
    }

    pub fn clear(&mut self) {
        self.owner.clear();
        self.held.clear();
    }

    /// Apply a mover's step events: on arrival it releases everything it held except the hex
    /// it arrived in (and any hex it already started stepping into in the same update). The
    /// hex it leaves stays held until arrival, so nobody walks into its back mid-step.
    pub fn track(&mut self, id: u32, events: &[Step]) {
        for (k, e) in events.iter().enumerate() {
            if let Step::Arrived(at) = e {
                let mut keep = vec![*at];
                for later in &events[k + 1..] {
                    if let Step::Left(_, to) = later {
                        keep.push(*to);
                    }
                }
                let drop: Vec<Hex> = self.held_by(id).iter().copied().filter(|h| !keep.contains(h)).collect();
                for h in drop {
                    self.release(h, id);
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.owner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.owner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knight_hex::{Layout, shapes};

    fn world() -> HexWorld {
        let mut w = HexWorld::new(Layout::pointy(1.0));
        for h in shapes::hexagon(Hex::ORIGIN, 5) {
            w.set_tile(h, crate::Tile::new(0, 0));
        }
        w
    }

    #[test]
    fn rejects_non_hex_routes() {
        let mut m = HexMover::new(Hex::ORIGIN, 2.0);
        assert_eq!(m.set_route(&[Hex::new(3, 0)]), Err(NavError::Disconnected));
        assert_eq!(
            m.set_route(&[Hex::ORIGIN, Hex::new(1, 0), Hex::new(3, 0)]),
            Err(NavError::NotAdjacent { index: 2 })
        );
        assert!(m.set_route(&[Hex::ORIGIN, Hex::new(1, 0), Hex::new(2, 0)]).is_ok());
    }

    #[test]
    fn walks_hex_to_hex_and_only_stops_on_centres() {
        let w = world();
        let mut m = HexMover::new(Hex::ORIGIN, 2.0);
        m.set_route(&[Hex::ORIGIN, Hex::new(1, 0), Hex::new(2, 0), Hex::new(3, 0)]).unwrap();
        let mut arrived = Vec::new();
        for _ in 0..30 {
            for e in m.update(0.05, |_, _| Some(1.0)) {
                if let Step::Arrived(h) = e {
                    arrived.push(h);
                }
            }
            if arrived.len() == 1 {
                m.stop();
            }
        }
        // Stopped at the end of the step it was on, exactly on a hex centre.
        assert_eq!(arrived, vec![Hex::new(1, 0), Hex::new(2, 0)]);
        assert!(!m.is_moving());
        assert!((m.position(&w) - w.hex_to_world(Hex::new(2, 0))).length() < 1e-5);
    }

    #[test]
    fn reservations_prevent_overlap() {
        let mut res = Reservations::new();
        let mut a = HexMover::new(Hex::ORIGIN, 4.0);
        let mut b = HexMover::new(Hex::new(2, 0), 4.0);
        res.reserve(Hex::ORIGIN, 1);
        res.reserve(Hex::new(2, 0), 2);
        a.set_route(&[Hex::ORIGIN, Hex::new(1, 0), Hex::new(2, 0)]).unwrap();
        b.set_route(&[Hex::new(2, 0), Hex::new(1, 0)]).unwrap();
        for _ in 0..40 {
            let ev = a.update(0.05, |_, to| res.reserve(to, 1).then_some(1.0));
            res.track(1, &ev);
            let ev = b.update(0.05, |_, to| res.reserve(to, 2).then_some(1.0));
            res.track(2, &ev);
        }
        // One of them got (1,0); the other waited. They never share a hex.
        assert_ne!(a.hex(), b.hex());
        assert!(a.blocked_for > 0.0 || b.blocked_for > 0.0);
    }

    /// Arriving and starting the next step in one update keeps the new reservation.
    #[test]
    fn fast_movers_hold_exactly_their_hexes() {
        let mut res = Reservations::new();
        let mut m = HexMover::new(Hex::ORIGIN, 3.0);
        res.reserve(Hex::ORIGIN, 1);
        let route: Vec<Hex> = (0..6).map(|q| Hex::new(q, 0)).collect();
        m.set_route(&route).unwrap();
        for _ in 0..20 {
            let ev = m.update(0.4, |_, to| res.reserve(to, 1).then_some(1.0));
            res.track(1, &ev);
            let held = res.held_by(1);
            assert!(held.contains(&m.next_stop()));
            assert!(held.len() <= 2, "holds {held:?}");
        }
        assert_eq!(res.held_by(1), &[Hex::new(5, 0)]);
    }
}
