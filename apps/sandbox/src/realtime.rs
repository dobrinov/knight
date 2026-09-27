//! Real-time hex tactics shared by the real-time demos.
//!
//! Everything still happens on the grid: units stand on hexes and move hex to hex with
//! [`HexMover`]; before stepping into a hex a unit must reserve it ([`Reservations`]), so units
//! never overlap and queue behind each other. Terrain sets the speed of every step. Orders are
//! "move to a hex" (groups spread over distinct target hexes) or "attack a unit" (chase until in
//! range, then attack on a cooldown). Idle units defend themselves against enemies nearby.

use knight_engine::glam::Vec3;
use knight_engine::hex::path;
use knight_engine::hex::{Hex, HexSet, Rng};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{Actor, Anim};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Idle,
    Move(Hex),
    Attack(usize),
}

#[derive(Clone, Debug)]
pub struct RtUnit {
    pub actor: Actor,
    pub range: i32,
    pub damage: (i32, i32),
    /// Seconds between attacks.
    pub rate: f32,
    pub cooldown: f32,
    pub order: Order,
    /// Hexes within which idle units notice enemies.
    pub aggro: i32,
    repath: f32,
    released: bool,
}

impl RtUnit {
    pub fn team(&self) -> Team {
        self.actor.team
    }

    pub fn alive(&self) -> bool {
        self.actor.alive()
    }

    pub fn hex(&self) -> Hex {
        self.actor.hex()
    }
}

pub struct Shot {
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
}

pub struct Floater {
    pub pos: Vec3,
    pub text: String,
    pub color: Color,
    pub t: f32,
}

/// Something the game may react to.
#[derive(Clone, Copy, Debug)]
pub enum RtEvent {
    Killed {
        by: usize,
    },
    /// A unit was hit at `pos`; `ranged` for arrows and spells.
    Struck {
        pos: Vec3,
        ranged: bool,
    },
}

#[derive(Default)]
pub struct RtWorld {
    pub units: Vec<RtUnit>,
    pub reservations: Reservations,
    pub shots: Vec<Shot>,
    pub floaters: Vec<Floater>,
    /// Simulation speed: 0 = paused, 1 = normal, 2, 4...
    pub speed: f32,
    rng: Option<Rng>,
}

fn id(i: usize) -> u32 {
    i as u32 + 1
}

impl RtWorld {
    pub fn new() -> Self {
        RtWorld { speed: 1.0, rng: Some(Rng::new(99)), ..Default::default() }
    }

    fn rng(&mut self) -> &mut Rng {
        self.rng.get_or_insert_with(|| Rng::new(99))
    }

    /// Add a unit on `hex` (if free). Returns its index.
    pub fn spawn(&mut self, world: &HexWorld, kind: UnitKind, team: Team, hex: Hex) -> Option<usize> {
        let i = self.units.len();
        if !world.contains(hex) || !self.reservations.reserve(hex, id(i)) {
            return None;
        }
        let (hp, range, damage, rate, speed) = match kind {
            UnitKind::Footman => (40.0, 1, (5, 8), 1.0, 1.8),
            UnitKind::Archer => (26.0, 4, (4, 6), 1.3, 1.8),
            UnitKind::Knight => (60.0, 1, (8, 12), 1.1, 3.0),
            UnitKind::Ogre => (70.0, 1, (7, 11), 1.4, 1.3),
            UnitKind::Griffon => (90.0, 1, (12, 18), 1.0, 3.2),
            UnitKind::Paladin => (120.0, 1, (9, 15), 0.7, 2.4),
            UnitKind::Wolf => (22.0, 1, (3, 6), 0.9, 2.6),
        };
        let mut actor = Actor::new(kind, team, world, hex);
        actor.hp = hp;
        actor.max_hp = hp;
        actor.mover.speed = speed;
        actor.scale = 0.85;
        self.units.push(RtUnit {
            actor,
            range,
            damage,
            rate,
            cooldown: 0.0,
            order: Order::Idle,
            aggro: if team == Team::Red { 5 } else { 4 },
            repath: 0.0,
            released: false,
        });
        Some(i)
    }

    /// Hexes held by units (standing or stepping).
    pub fn held(&self, h: Hex) -> Option<usize> {
        self.reservations.owner(h).map(|o| o as usize - 1)
    }

    /// Unit standing on (or entering) hex `h`.
    pub fn unit_at(&self, h: Hex) -> Option<usize> {
        self.held(h).filter(|&i| self.units[i].alive())
    }

    /// Order a group to move: the closest units take the free hexes closest to the goal.
    pub fn order_move(&mut self, world: &HexWorld, group: &[usize], goal: Hex) {
        let mut slots: Vec<Hex> = goal
            .spiral(((group.len() as f32).sqrt().ceil() as i32 + 1).max(1))
            .into_iter()
            .filter(|&h| world.move_cost(h).is_some() && !world.is_submerged(h))
            .filter(|&h| self.held(h).is_none_or(|o| group.contains(&o) || !self.units[o].alive()))
            .collect();
        slots.sort_by_key(|h| (h.distance(goal), h.q, h.r));
        let mut members: Vec<usize> = group.iter().copied().filter(|&i| self.units[i].alive()).collect();
        members.sort_by_key(|&i| self.units[i].hex().distance(goal));
        for (k, &i) in members.iter().enumerate() {
            let target = slots.get(k).copied().unwrap_or(goal);
            let u = &mut self.units[i];
            u.order = Order::Move(target);
            u.repath = 0.0;
            u.actor.mover.stop();
        }
    }

    pub fn order_attack(&mut self, group: &[usize], target: usize) {
        for &i in group {
            let u = &mut self.units[i];
            if u.alive() && i != target {
                u.order = Order::Attack(target);
                u.repath = 0.0;
            }
        }
    }

    /// A* route for unit `i` to `goal` (stopping as soon as `done(hex)`), treating hexes held by
    /// others as expensive so units route around crowds instead of waiting forever.
    fn plan(&self, world: &HexWorld, i: usize, goal: Hex, done: impl Fn(Hex) -> bool) -> Option<Vec<Hex>> {
        let me = id(i);
        let from = self.units[i].actor.mover.next_stop();
        let climb = self.units[i].actor.climb;
        let p = path::astar(from, goal, 1, 400, |a, b| {
            let c = world.step_points(a, b, climb, 4)?;
            Some(if self.reservations.is_free(b, me) || b == goal { c } else { c + 24 })
        })?;
        let mut hexes = p.hexes;
        if let Some(k) = hexes.iter().position(|&h| done(h)) {
            hexes.truncate(k + 1);
        }
        // Never try to walk into a held goal hex; stop next to it.
        while hexes.len() > 1 && !self.reservations.is_free(*hexes.last().unwrap(), me) {
            hexes.pop();
        }
        Some(hexes)
    }

    /// Advance the simulation (scaled by `speed`; nothing moves while paused).
    pub fn step(&mut self, world: &HexWorld, art: &Art, dt: f32) -> Vec<RtEvent> {
        let dt = dt * self.speed;
        let mut events = Vec::new();
        if dt <= 0.0 {
            return events;
        }
        let snap: Vec<(Hex, Team, bool)> = self.units.iter().map(|u| (u.hex(), u.team(), u.alive())).collect();
        let mut hits: Vec<(usize, usize)> = Vec::new();
        for i in 0..self.units.len() {
            if !self.units[i].alive() {
                if !self.units[i].released {
                    // The dead free their hexes.
                    self.units[i].released = true;
                    self.reservations.release_all(id(i));
                }
                self.units[i].actor.update(world, art, dt);
                continue;
            }
            let (me_hex, me_team, _) = snap[i];
            self.units[i].cooldown -= dt;
            self.units[i].repath -= dt;
            // Idle units defend themselves.
            if self.units[i].order == Order::Idle {
                let aggro = self.units[i].aggro;
                let foe = (0..snap.len())
                    .filter(|&j| snap[j].2 && snap[j].1 != me_team && snap[j].0.distance(me_hex) <= aggro)
                    .min_by_key(|&j| snap[j].0.distance(me_hex));
                if let Some(f) = foe {
                    self.units[i].order = Order::Attack(f);
                    self.units[i].repath = 0.0;
                }
            }
            match self.units[i].order {
                Order::Attack(t) if !snap[t].2 => {
                    self.units[i].order = Order::Idle;
                    self.units[i].actor.mover.stop();
                }
                Order::Attack(t) => {
                    let (th, _, _) = snap[t];
                    let range = self.units[i].range;
                    let standing = self.units[i].actor.mover.step().is_none();
                    if me_hex.distance(th) <= range && standing {
                        self.units[i].actor.mover.stop();
                        let u = &mut self.units[i];
                        u.actor.facing_left = world.hex_to_world(th).x < u.actor.pos.x;
                        if u.cooldown <= 0.0 {
                            u.cooldown = u.rate;
                            u.actor.play(if range > 1 { Anim::Shoot } else { Anim::Attack });
                            hits.push((i, t));
                        }
                    } else if self.units[i].repath <= 0.0 || !self.units[i].actor.moving() {
                        self.units[i].repath = 0.5;
                        let route = self.plan(world, i, th, |h| h.distance(th) <= range);
                        if let Some(r) = route {
                            self.units[i].actor.walk_hexes(&r);
                        }
                    }
                }
                Order::Move(goal) => {
                    let u = &self.units[i];
                    if u.actor.mover.next_stop() == goal && u.actor.mover.step().is_none() {
                        self.units[i].order = Order::Idle;
                    } else if (!u.actor.moving() || u.actor.mover.blocked_for > 0.5) && self.units[i].repath <= 0.0 {
                        self.units[i].repath = 0.4;
                        match self.plan(world, i, goal, |h| h == goal) {
                            Some(r) if r.len() > 1 => self.units[i].actor.walk_hexes(&r),
                            // Goal taken or unreachable: good enough, stop here.
                            _ => self.units[i].order = Order::Idle,
                        }
                    }
                }
                Order::Idle => {}
            }
            // Move: terrain sets the speed; only reserved hexes may be entered.
            let (res, u) = (&mut self.reservations, &mut self.units[i]);
            let me = id(i);
            let events_i = u.actor.update_with(world, art, dt, |a, b| {
                let cost = world.step_cost(a, b, 1)?;
                res.reserve(b, me).then_some(cost)
            });
            res.track(me, &events_i);
        }
        for (a, t) in hits {
            if !self.units[t].alive() {
                continue;
            }
            let (lo, hi) = self.units[a].damage;
            let d = self.rng().range(lo, hi + 1);
            let (from, to) = (self.units[a].actor.pos, self.units[t].actor.pos);
            if self.units[a].range > 1 {
                self.shots.push(Shot { from: from + Vec3::Y * 0.8, to: to + Vec3::Y * 0.8, t: 0.0 });
            }
            events.push(RtEvent::Struck { pos: to, ranged: self.units[a].range > 1 });
            let tu = &mut self.units[t];
            tu.actor.hp -= d as f32;
            let col = if tu.team() == Team::Blue { Color::hex(0xff7a6a) } else { Color::WHITE };
            self.floaters.push(Floater { pos: to + Vec3::Y * 1.5, text: format!("-{d}"), color: col, t: 0.0 });
            if tu.alive() {
                if tu.order == Order::Idle {
                    tu.order = Order::Attack(a);
                }
                if !matches!(tu.actor.anim, Anim::Attack | Anim::Shoot) {
                    tu.actor.play(Anim::Hurt);
                }
            } else {
                tu.actor.play(Anim::Death);
                events.push(RtEvent::Killed { by: a });
            }
        }
        for s in &mut self.shots {
            s.t += dt;
        }
        self.shots.retain(|s| s.t < 0.3);
        for f in &mut self.floaters {
            f.t += dt;
            f.pos.y += dt * 0.8;
        }
        self.floaters.retain(|f| f.t < 1.1);
        events
    }

    /// Damage every enemy of `by` within `radius` hexes of it.
    pub fn blast(&mut self, by: usize, radius: i32, damage: (i32, i32)) -> Vec<RtEvent> {
        let center = self.units[by].hex();
        let team = self.units[by].team();
        let mut events = Vec::new();
        for t in 0..self.units.len() {
            if !self.units[t].alive() || self.units[t].team() == team || self.units[t].hex().distance(center) > radius {
                continue;
            }
            let d = self.rng().range(damage.0, damage.1 + 1);
            let tu = &mut self.units[t];
            tu.actor.hp -= d as f32;
            let pos = tu.actor.pos;
            events.push(RtEvent::Struck { pos, ranged: true });
            self.floaters.push(Floater {
                pos: pos + Vec3::Y * 1.5,
                text: format!("-{d}"),
                color: Color::hex(0x9fd8ff),
                t: 0.0,
            });
            if self.units[t].alive() {
                self.units[t].actor.play(Anim::Hurt);
            } else {
                self.units[t].actor.play(Anim::Death);
                events.push(RtEvent::Killed { by });
            }
        }
        events
    }

    pub fn draw_fx(&self, w: &mut WorldDraw, art: &Art) {
        for s in &self.shots {
            let k = (s.t / 0.3).clamp(0.0, 1.0);
            let p = s.from.lerp(s.to, k) + Vec3::Y * (k * (1.0 - k) * 1.2);
            w.sprite(Sprite::new(art.arrow, p).anchor(0.5, 0.5).scale(0.5).flip(s.to.x < s.from.x));
        }
        for f in &self.floaters {
            w.label(f.pos, &f.text, 16.0, f.color.with_alpha((1.1 - f.t).clamp(0.0, 1.0)));
        }
    }

    /// Hexes seen by `team` (simple radius vision).
    pub fn vision(&self, team: Team, radius: i32) -> HexSet {
        let mut out = HexSet::default();
        for u in self.units.iter().filter(|u| u.team() == team && u.alive()) {
            out.extend(u.hex().spiral(radius));
        }
        out
    }
}

/// Real-time with pause: Space pauses, 1/2/3 pick normal, double and quadruple speed.
pub fn time_controls(ctx: &Context, rt: &mut RtWorld, paused_speed: &mut f32) {
    let i = &ctx.input;
    if i.key_pressed(Key::Space) || i.ui_clicked("pause") {
        if rt.speed > 0.0 {
            *paused_speed = rt.speed;
            rt.speed = 0.0;
        } else {
            rt.speed = paused_speed.max(1.0);
        }
    }
    for (k, v) in [(Key::Digit1, 1.0), (Key::Digit2, 2.0), (Key::Digit3, 4.0)] {
        if i.key_pressed(k) && !i.ctrl() {
            rt.speed = v;
        }
    }
    for (id, v) in [("speed1", 1.0), ("speed2", 2.0), ("speed4", 4.0)] {
        if i.ui_clicked(id) {
            rt.speed = v;
        }
    }
}

pub fn draw_time_controls(f: &mut Frame, rt: &RtWorld, at: knight_engine::glam::Vec2) {
    let r = Rect::new(at.x, at.y, 200.0, 34.0);
    f.panel(r);
    let cells = r.inset(5.0).cols(4, 4.0);
    f.button_ex("pause", cells[0], if rt.speed == 0.0 { ">" } else { "||" }, true, rt.speed == 0.0);
    f.button_ex("speed1", cells[1], "1x", true, rt.speed == 1.0);
    f.button_ex("speed2", cells[2], "2x", true, rt.speed == 2.0);
    f.button_ex("speed4", cells[3], "4x", true, rt.speed == 4.0);
    if rt.speed == 0.0 {
        let size = f.ui_size();
        let accent = f.theme.accent;
        f.text_centered(Rect::new(0.0, 70.0, size.x, 30.0), "PAUSED - give orders, Space to resume", 16.0, accent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use knight_engine::hex::{Layout, shapes};

    fn flat() -> HexWorld {
        let mut w = HexWorld::new(Layout::pointy(1.0));
        for h in shapes::hexagon(Hex::ORIGIN, 8) {
            w.set_tile(h, Tile::new(1, 0));
        }
        w
    }

    /// A group converges on distinct hexes, never sharing one, and a fight resolves.
    #[test]
    fn group_moves_without_overlap_and_fights() {
        let mut ctx = Context::new();
        let art = Art::load(&mut ctx);
        let world = flat();
        let mut rt = RtWorld::new();
        let group: Vec<usize> =
            (0..8).filter_map(|k| rt.spawn(&world, UnitKind::Footman, Team::Blue, Hex::new(-6, k - 2))).collect();
        rt.order_move(&world, &group, Hex::new(4, -1));
        for _ in 0..600 {
            rt.step(&world, &art, 1.0 / 30.0);
            let mut seen = std::collections::HashMap::new();
            for (k, u) in rt.units.iter().enumerate() {
                if let Some(o) = seen.insert(u.hex(), k) {
                    let o: usize = o;
                    panic!(
                        "two units on {:?}: {:?} step {:?} next {:?} / {:?} step {:?} next {:?}; owner {:?}",
                        u.hex(),
                        k,
                        u.actor.mover.step(),
                        u.actor.mover.next_stop(),
                        o,
                        rt.units[o].actor.mover.step(),
                        rt.units[o].actor.mover.next_stop(),
                        rt.reservations.owner(u.hex())
                    );
                }
            }
        }
        assert!(rt.units.iter().all(|u| u.hex().distance(Hex::new(4, -1)) <= 3), "group arrived");
        let wolf = rt.spawn(&world, UnitKind::Wolf, Team::Red, Hex::new(-2, 0)).unwrap();
        rt.order_attack(&group, wolf);
        for _ in 0..900 {
            rt.step(&world, &art, 1.0 / 30.0);
        }
        assert!(!rt.units[wolf].alive(), "wolf died");
    }
}
