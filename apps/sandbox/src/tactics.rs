//! Turn-based hex tactics shared by the RPG and strategy demos: units that occupy hexes, movement
//! ranges, melee/ranged attacks, a simple enemy AI and a director that plays actions one after
//! another with animations.

use std::collections::VecDeque;

use knight_engine::glam::Vec3;
use knight_engine::hex::path::{self, Reachable};
use knight_engine::hex::{FlowField, Hex, HexMap, Rng};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{Actor, Anim};

/// A unit standing on a hex.
#[derive(Clone, Debug)]
pub struct Unit {
    pub actor: Actor,
    pub hex: Hex,
    /// Movement points per turn.
    pub moves: u32,
    /// Attack range in hexes (1 = melee).
    pub range: i32,
    pub damage: (i32, i32),
    /// Already moved / attacked this turn.
    pub moved: bool,
    pub acted: bool,
    /// Experience awarded for killing it.
    pub xp: u32,
    /// Units of different factions are enemies (two teams, or free-for-all with one each).
    pub faction: u32,
}

impl Unit {
    pub fn new(world: &HexWorld, kind: UnitKind, team: Team, hex: Hex) -> Unit {
        let (hp, moves, range, damage, xp) = match kind {
            UnitKind::Footman => (22.0, 3, 1, (5, 8), 10),
            UnitKind::Archer => (14.0, 3, 3, (4, 6), 10),
            UnitKind::Knight => (30.0, 5, 1, (8, 12), 20),
            UnitKind::Ogre => (32.0, 2, 1, (6, 10), 25),
            UnitKind::Griffon => (50.0, 6, 1, (12, 18), 50),
            UnitKind::Paladin => (60.0, 4, 1, (8, 14), 0),
            UnitKind::Wolf => (14.0, 4, 1, (3, 6), 8),
        };
        let mut actor = Actor::new(kind, team, world, hex);
        actor.hp = hp;
        actor.max_hp = hp;
        actor.mover.speed = 3.2;
        Unit { actor, hex, moves, range, damage, moved: false, acted: false, xp, faction: team as u32 }
    }

    pub fn team(&self) -> Team {
        self.actor.team
    }

    pub fn alive(&self) -> bool {
        self.actor.alive()
    }

    pub fn done(&self) -> bool {
        self.moved && self.acted
    }
}

/// Movement points for a normal step (terrain scales it: roads 5, forests 20, ...).
pub const POINTS: u32 = 10;

/// Movement points of one step for walkers: the world's terrain cost (see
/// [`HexWorld::step_points`]), no water, at most one level up or down.
pub fn step_cost(world: &HexWorld, from: Hex, to: Hex) -> Option<u32> {
    world.step_points(from, to, 1, POINTS)
}

pub fn occupancy(units: &[Unit]) -> HexMap<usize> {
    units.iter().enumerate().filter(|(_, u)| u.alive()).map(|(i, u)| (u.hex, i)).collect()
}

/// Hexes unit `i` can reach this turn with `moves` normal steps' worth of movement points
/// (terrain makes steps cheaper or dearer; occupied hexes block).
pub fn reach(world: &HexWorld, units: &[Unit], i: usize, moves: u32) -> Reachable {
    let occ = occupancy(units);
    path::reachable(units[i].hex, moves * POINTS, |a, b| if occ.contains(b) { None } else { step_cost(world, a, b) })
}

/// Can `i` hit `t` from hex `from`?
pub fn in_range(units: &[Unit], i: usize, from: Hex, t: usize) -> bool {
    let d = from.distance(units[t].hex);
    d >= 1 && d <= units[i].range
}

/// A thing that happens on the board, played by the [`Director`].
#[derive(Clone, Debug)]
pub enum Action {
    Move {
        unit: usize,
        path: Vec<Hex>,
    },
    Attack {
        unit: usize,
        target: usize,
    },
    /// Damage every enemy of `unit` within `radius` of it.
    Nova {
        unit: usize,
        radius: i32,
        damage: (i32, i32),
    },
}

/// Something the game may want to react to.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    Killed {
        unit: usize,
        by: usize,
    },
    /// `target` was hit (at world `pos`); `ranged` if by a projectile or spell.
    Struck {
        pos: Vec3,
        ranged: bool,
    },
}

struct Running {
    action: Action,
    t: f32,
    done_effect: bool,
}

pub struct Floater {
    pub pos: Vec3,
    pub text: String,
    pub color: Color,
    pub t: f32,
}

/// Plays queued actions with animations. Moves start immediately and run in parallel; attacks
/// and spells play one at a time, each waiting until the units involved have stopped walking.
pub struct Director {
    queue: VecDeque<Action>,
    running: Option<Running>,
    walking: Vec<usize>,
    pub floaters: Vec<Floater>,
    rng: Rng,
    /// Animation speed multiplier.
    pub speed: f32,
}

impl Director {
    pub fn new(seed: u64) -> Self {
        Director {
            queue: VecDeque::new(),
            running: None,
            walking: Vec::new(),
            floaters: Vec::new(),
            rng: Rng::new(seed),
            speed: 1.0,
        }
    }

    pub fn push(&mut self, a: Action) {
        self.queue.push_back(a);
    }

    pub fn busy(&self) -> bool {
        self.running.is_some() || !self.queue.is_empty() || !self.walking.is_empty()
    }

    pub fn float(&mut self, pos: Vec3, text: String, color: Color) {
        self.floaters.push(Floater { pos: pos + Vec3::Y * 1.5, text, color, t: 0.0 });
    }

    fn damage(&mut self, units: &mut [Unit], by: usize, target: usize, dmg: (i32, i32), events: &mut Vec<Event>) {
        if !units[target].alive() {
            return;
        }
        let d = self.rng.range(dmg.0, dmg.1 + 1);
        let t = &mut units[target];
        t.actor.hp -= d as f32;
        t.actor.play(if t.alive() { Anim::Hurt } else { Anim::Death });
        let pos = t.actor.pos;
        let ranged = units[by].range > 1 || units[by].hex.distance(units[target].hex) > 1;
        events.push(Event::Struck { pos, ranged });
        let t = &units[target];
        let col = if t.team() == Team::Blue { Color::hex(0xff7a6a) } else { Color::WHITE };
        self.float(pos, format!("-{d}"), col);
        if !units[target].alive() {
            events.push(Event::Killed { unit: target, by });
        }
    }

    /// Advance animations and actions. Returns what happened this frame.
    pub fn update(&mut self, world: &HexWorld, art: &Art, units: &mut [Unit], dt: f32) -> Vec<Event> {
        let dt = dt * self.speed;
        let mut events = Vec::new();
        for u in units.iter_mut() {
            u.actor.update(world, art, dt);
        }
        self.walking.retain(|&i| units[i].actor.moving());
        while self.running.is_none() {
            let Some(a) = self.queue.pop_front() else { break };
            // Skip actions of units that died meanwhile.
            let valid = match &a {
                Action::Move { unit, path } => units[*unit].alive() && path.len() > 1,
                Action::Attack { unit, target } => units[*unit].alive() && units[*target].alive(),
                Action::Nova { unit, .. } => units[*unit].alive(),
            };
            if !valid {
                continue;
            }
            match &a {
                Action::Move { unit, path } => {
                    units[*unit].actor.walk_hexes(path);
                    units[*unit].hex = *path.last().unwrap();
                    self.walking.push(*unit);
                    continue;
                }
                Action::Attack { unit, target } => {
                    if self.walking.contains(unit) || self.walking.contains(target) {
                        self.queue.push_front(a);
                        break;
                    }
                    let (from, to) = (units[*unit].actor.pos, units[*target].actor.pos);
                    let u = &mut units[*unit];
                    u.actor.facing_left = to.x < from.x;
                    u.actor.play(if u.range > 1 { Anim::Shoot } else { Anim::Attack });
                }
                Action::Nova { unit, .. } => {
                    if self.walking.contains(unit) {
                        self.queue.push_front(a);
                        break;
                    }
                    units[*unit].actor.play(Anim::Attack);
                }
            }
            self.running = Some(Running { action: a, t: 0.0, done_effect: false });
        }
        let mut finished = false;
        if let Some(r) = &mut self.running {
            r.t += dt;
            let (t, done_effect) = (r.t, r.done_effect);
            match r.action.clone() {
                Action::Move { .. } => finished = true,
                Action::Attack { unit, target } => {
                    if !done_effect && t >= 0.3 {
                        r.done_effect = true;
                        let dmg = units[unit].damage;
                        self.damage(units, unit, target, dmg, &mut events);
                    }
                    finished = t >= 0.6;
                }
                Action::Nova { unit, radius, damage } => {
                    if !done_effect && t >= 0.2 {
                        r.done_effect = true;
                        let faction = units[unit].faction;
                        let center = units[unit].hex;
                        let hit: Vec<usize> = (0..units.len())
                            .filter(|&j| {
                                units[j].alive()
                                    && units[j].faction != faction
                                    && units[j].hex.distance(center) <= radius
                            })
                            .collect();
                        for j in hit {
                            self.damage(units, unit, j, damage, &mut events);
                        }
                    }
                    finished = t >= 0.6;
                }
            }
        }
        if finished {
            self.running = None;
        }
        for f in &mut self.floaters {
            f.t += dt;
            f.pos.y += dt * 0.8;
        }
        self.floaters.retain(|f| f.t < 1.2);
        events
    }

    /// Projectiles, spell rings and damage numbers.
    pub fn draw(&self, w: &mut WorldDraw, art: &Art, units: &[Unit]) {
        if let Some(r) = &self.running {
            match r.action {
                Action::Attack { unit, target } if units[unit].range > 1 => {
                    let k = ((r.t - 0.05) / 0.25).clamp(0.0, 1.0);
                    if k > 0.0 && k < 1.0 {
                        let (a, b) = (units[unit].actor.pos + Vec3::Y * 0.9, units[target].actor.pos + Vec3::Y * 0.9);
                        let p = a.lerp(b, k) + Vec3::Y * (k * (1.0 - k) * 1.5);
                        w.sprite(Sprite::new(art.arrow, p).anchor(0.5, 0.5).scale(0.6).flip(b.x < a.x));
                    }
                }
                Action::Nova { unit, radius, .. } => {
                    let k = (r.t / 0.6).clamp(0.0, 1.0);
                    let p = units[unit].actor.pos;
                    let reach = radius as f32 * 1.75;
                    w.ring(p, 0.4 + k * reach, Color::hex(0x8fd0ff).with_alpha(1.0 - k));
                    w.ring(p, 0.2 + k * reach * 0.85, Color::WHITE.with_alpha(0.7 * (1.0 - k)));
                }
                _ => {}
            }
        }
        for f in &self.floaters {
            w.label(f.pos, &f.text, 16.0, f.color.with_alpha((1.2 - f.t).clamp(0.0, 1.0)));
        }
    }
}

/// Plan an AI turn for unit `i`: attack something in range, otherwise walk towards the nearest
/// enemy (following a flow field so it goes around obstacles) and attack if it gets close.
pub fn ai_turn(world: &HexWorld, units: &[Unit], i: usize) -> Vec<Action> {
    ai_turn_with(world, units, i, &|_| true, None, None)
}

/// [`ai_turn`] with a hazard: `safe(hex)` is false where standing hurts (fire, gas, lava); the
/// unit never ends its move there if it can help it and heads for `refuge` when in danger or
/// with no enemy in `sight` (hexes; `None` = sees everything).
pub fn ai_turn_with(
    world: &HexWorld,
    units: &[Unit],
    i: usize,
    safe: &dyn Fn(Hex) -> bool,
    refuge: Option<Hex>,
    sight: Option<i32>,
) -> Vec<Action> {
    let me = &units[i];
    let foes: Vec<usize> = (0..units.len())
        .filter(|&j| units[j].alive() && units[j].faction != me.faction)
        .filter(|&j| sight.is_none_or(|s| units[j].hex.distance(me.hex) <= s))
        .collect();
    let weakest_in_range = |from: Hex| {
        foes.iter()
            .copied()
            .filter(|&f| in_range(units, i, from, f))
            .min_by(|&a, &b| units[a].actor.hp.total_cmp(&units[b].actor.hp))
    };
    let in_danger = !safe(me.hex);
    if !in_danger && let Some(t) = weakest_in_range(me.hex) {
        return vec![Action::Attack { unit: i, target: t }];
    }
    let r = reach(world, units, i, me.moves);
    // Where to go: towards enemies, or towards the refuge when in danger / alone.
    let goals: Vec<Hex> = if in_danger || foes.is_empty() {
        refuge.into_iter().collect()
    } else {
        foes.iter().map(|&f| units[f].hex).collect()
    };
    if goals.is_empty() {
        return Vec::new();
    }
    // Distance field to the goals (ignoring units, so crowds don't hide routes).
    let field = FlowField::build(goals, 120 * POINTS, |a, b| step_cost(world, a, b));
    let score = |h: Hex| {
        let attack = weakest_in_range(h).is_some();
        (!safe(h), !attack, field.cost(h).unwrap_or(u32::MAX), r.cost(h).unwrap_or(0))
    };
    let best = r.hexes().min_by_key(|&h| score(h)).unwrap_or(me.hex);
    let mut out = Vec::new();
    if best != me.hex && score(best) < score(me.hex) {
        out.push(Action::Move { unit: i, path: r.path_to(best).unwrap_or_default() });
    }
    if let Some(t) = weakest_in_range(best) {
        out.push(Action::Attack { unit: i, target: t });
    }
    out
}

/// The command a player click would give unit `i`: move to a hex, attack an enemy in range, or
/// walk next to an enemy and attack it.
pub fn player_command(world: &HexWorld, units: &[Unit], i: usize, r: &Reachable, click: Pick) -> Vec<Action> {
    let occ = occupancy(units);
    let u = &units[i];
    if let Some(&t) = occ.get(click.hex) {
        if units[t].faction == u.faction || u.acted {
            return Vec::new();
        }
        if in_range(units, i, u.hex, t) {
            return vec![Action::Attack { unit: i, target: t }];
        }
        if u.moved {
            return Vec::new();
        }
        // Step to the reachable hex in range of the target that is closest to the cursor.
        let best = r.hexes().filter(|&h| in_range(units, i, h, t)).min_by(|&a, &b| {
            let (pa, pb) = (world.hex_to_world(a), world.hex_to_world(b));
            pa.distance(click.point).total_cmp(&pb.distance(click.point))
        });
        return match best {
            Some(h) => vec![
                Action::Move { unit: i, path: r.path_to(h).unwrap_or_default() },
                Action::Attack { unit: i, target: t },
            ],
            None => Vec::new(),
        };
    }
    if !u.moved && click.hex != u.hex && r.contains(click.hex) {
        return vec![Action::Move { unit: i, path: r.path_to(click.hex).unwrap_or_default() }];
    }
    Vec::new()
}
