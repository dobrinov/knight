//! Closing-in hazards for battle royale and "the map shrinks" game modes.
//!
//! [`ShrinkingZone`] is the rule: a safe circle of hexes that shrinks in phases towards a point
//! chosen at random inside the previous circle, so players are pushed together and can plan
//! ahead (the next circle is known before it closes). [`Wildfire`] is the look and the terrain
//! effect: fire that spreads hex to hex by flammability, burns for a while and leaves ash. Drive
//! the fire from the zone (ignite what falls outside it) and players see the danger coming.
//! Both advance in ticks — turns in a turn-based game, seconds in a real-time one.

use knight_hex::{FracHex, Hex, HexMap, HexSet, Rng};

/// One phase: wait, then shrink to `radius` over `shrink` ticks. Outside the zone costs
/// `damage` per tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZonePhase {
    pub wait: u32,
    pub shrink: u32,
    pub radius: i32,
    pub damage: f32,
}

/// What the zone did this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneEvent {
    /// Holding still; the next shrink starts in `ticks` ticks.
    Waiting {
        ticks: u32,
    },
    /// A new shrink started (announce it!).
    ShrinkStarted {
        phase: usize,
    },
    Shrinking,
    /// Reached the phase's circle.
    ShrinkEnded {
        phase: usize,
    },
    /// All phases done; the zone stays as it is.
    Final,
}

#[derive(Clone, Debug)]
pub struct ShrinkingZone {
    /// Current safe circle.
    pub center: Hex,
    pub radius: i32,
    phases: Vec<ZonePhase>,
    phase: usize,
    tick: u32,
    from: (Hex, i32),
    next: (Hex, i32),
    rng: Rng,
    /// Keeps chosen centres on valid ground (e.g. not in the sea).
    valid: Option<HexSet>,
}

impl ShrinkingZone {
    /// A zone starting as the circle `center`/`radius`. `valid` (optional) limits where later
    /// centres may be placed.
    pub fn new(center: Hex, radius: i32, phases: Vec<ZonePhase>, seed: u64, valid: Option<HexSet>) -> Self {
        let mut z = ShrinkingZone {
            center,
            radius,
            phases,
            phase: 0,
            tick: 0,
            from: (center, radius),
            next: (center, radius),
            rng: Rng::new(seed),
            valid,
        };
        z.pick_next();
        z
    }

    /// Choose the next circle inside the current one.
    fn pick_next(&mut self) {
        let Some(p) = self.phases.get(self.phase).copied() else {
            self.next = (self.center, self.radius);
            return;
        };
        let slack = (self.radius - p.radius).max(0);
        let mut best = self.center;
        for _ in 0..40 {
            let c = self.center + Hex::new(self.rng.range(-slack, slack + 1), self.rng.range(-slack, slack + 1));
            if c.distance(self.center) <= slack && self.valid.as_ref().is_none_or(|v| v.contains(&c)) {
                best = c;
                break;
            }
        }
        self.from = (self.center, self.radius);
        self.next = (best, p.radius);
    }

    /// Is `h` inside the current safe zone?
    pub fn contains(&self, h: Hex) -> bool {
        h.distance(self.center) <= self.radius
    }

    /// The circle the zone is heading to (known in advance so players can plan).
    pub fn next(&self) -> (Hex, i32) {
        self.next
    }

    /// Damage per tick outside the zone right now.
    pub fn damage(&self) -> f32 {
        let i = self.phase.min(self.phases.len().saturating_sub(1));
        self.phases.get(i).map_or(0.0, |p| p.damage)
    }

    pub fn phase(&self) -> usize {
        self.phase
    }

    pub fn is_final(&self) -> bool {
        self.phase >= self.phases.len()
    }

    /// Ticks until the current phase starts shrinking (0 while shrinking).
    pub fn ticks_until_shrink(&self) -> u32 {
        self.phases.get(self.phase).map_or(0, |p| p.wait.saturating_sub(self.tick))
    }

    pub fn is_shrinking(&self) -> bool {
        self.phases.get(self.phase).is_some_and(|p| self.tick >= p.wait)
    }

    /// Advance one tick.
    pub fn tick(&mut self) -> ZoneEvent {
        let Some(p) = self.phases.get(self.phase).copied() else { return ZoneEvent::Final };
        self.tick += 1;
        if self.tick <= p.wait {
            return if self.tick == p.wait {
                ZoneEvent::ShrinkStarted { phase: self.phase }
            } else {
                ZoneEvent::Waiting { ticks: p.wait - self.tick }
            };
        }
        let k = ((self.tick - p.wait) as f32 / p.shrink.max(1) as f32).min(1.0);
        // Move the centre and the radius together, one step at a time.
        let (fc, fr) = self.from;
        let (nc, nr) = self.next;
        self.center = FracHex::from(fc).lerp(nc.into(), k as f64).round();
        self.radius = (fr as f32 + (nr - fr) as f32 * k).round() as i32;
        if k >= 1.0 {
            self.center = nc;
            self.radius = nr;
            let done = self.phase;
            self.phase += 1;
            self.tick = 0;
            self.pick_next();
            return ZoneEvent::ShrinkEnded { phase: done };
        }
        ZoneEvent::Shrinking
    }
}

/// Fire spreading over hexes. Burning hexes burn for a few ticks (longer if more flammable)
/// and then turn to ash, which never burns again.
#[derive(Clone, Debug)]
pub struct Wildfire {
    burning: HexMap<u8>,
    ash: HexSet,
    rng: Rng,
}

impl Wildfire {
    pub fn new(seed: u64) -> Self {
        Wildfire { burning: HexMap::new(), ash: HexSet::default(), rng: Rng::new(seed) }
    }

    pub fn is_burning(&self, h: Hex) -> bool {
        self.burning.contains(h)
    }

    pub fn is_ash(&self, h: Hex) -> bool {
        self.ash.contains(&h)
    }

    pub fn burning(&self) -> impl Iterator<Item = Hex> + '_ {
        self.burning.hexes()
    }

    pub fn ash(&self) -> impl Iterator<Item = &Hex> {
        self.ash.iter()
    }

    /// Set a hex alight (if it can burn and hasn't already).
    pub fn ignite(&mut self, h: Hex, flammability: f32) -> bool {
        if flammability <= 0.0 || self.ash.contains(&h) || self.burning.contains(h) {
            return false;
        }
        self.burning.insert(h, (1.0 + flammability * 3.0).round() as u8);
        true
    }

    /// Advance: fire spreads to neighbours with chance `flammability(h)` (0..1) where
    /// `allowed(h)`, and burning hexes count down to ash. Returns the newly ignited hexes.
    pub fn tick(&mut self, flammability: impl Fn(Hex) -> f32, allowed: impl Fn(Hex) -> bool) -> Vec<Hex> {
        let mut fresh = Vec::new();
        let burning: Vec<Hex> = self.burning.hexes().collect();
        for &h in &burning {
            for n in h.neighbors() {
                let f = flammability(n);
                if allowed(n)
                    && f > 0.0
                    && !self.ash.contains(&n)
                    && !self.burning.contains(n)
                    && self.rng.chance(f * 0.6)
                {
                    fresh.push(n);
                }
            }
        }
        for h in burning {
            let left = self.burning.get_mut(h).unwrap();
            *left = left.saturating_sub(1);
            if *left == 0 {
                self.burning.remove(h);
                self.ash.insert(h);
            }
        }
        fresh.retain(|&h| self.ignite(h, flammability(h)));
        fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phases() -> Vec<ZonePhase> {
        vec![
            ZonePhase { wait: 2, shrink: 3, radius: 8, damage: 2.0 },
            ZonePhase { wait: 2, shrink: 2, radius: 3, damage: 5.0 },
        ]
    }

    #[test]
    fn zone_shrinks_inside_itself() {
        let mut z = ShrinkingZone::new(Hex::ORIGIN, 14, phases(), 9, None);
        let (nc, nr) = z.next();
        assert!(nc.distance(Hex::ORIGIN) + nr <= 14, "next circle fits inside the first");
        assert_eq!(z.tick(), ZoneEvent::Waiting { ticks: 1 });
        assert_eq!(z.tick(), ZoneEvent::ShrinkStarted { phase: 0 });
        let mut last = z.radius;
        let mut ended = false;
        for _ in 0..3 {
            let e = z.tick();
            assert!(z.radius <= last);
            last = z.radius;
            ended |= e == ZoneEvent::ShrinkEnded { phase: 0 };
        }
        assert!(ended);
        assert_eq!((z.center, z.radius), (nc, nr));
        for _ in 0..10 {
            z.tick();
        }
        assert!(z.is_final());
        assert_eq!(z.radius, 3);
        assert_eq!(z.damage(), 5.0);
    }

    #[test]
    fn fire_spreads_by_flammability_and_burns_out() {
        let mut f = Wildfire::new(1);
        f.ignite(Hex::ORIGIN, 1.0);
        // Only the east half may burn; the west is "safe".
        for _ in 0..30 {
            f.tick(|h| if h.length() <= 6 { 1.0 } else { 0.0 }, |h| h.q >= 0);
        }
        assert!(f.ash().count() > 5, "fire spread and burned out");
        assert!(f.ash().all(|h| h.q >= 0 && h.length() <= 6), "stayed where allowed");
        assert!(!f.ignite(Hex::ORIGIN, 1.0), "ash never reignites");
    }
}
