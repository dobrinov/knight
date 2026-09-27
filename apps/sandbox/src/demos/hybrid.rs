//! Hybrid turns: timed days instead of "End turn". You and three rival heroes act at the same
//! time, in real time, but each day everyone only has a budget of movement points (terrain
//! costs them) and actions (attacks). Spend them before the day's timer runs out; then wait for
//! dawn. Everyone declaring "Ready" ends the day early. Race the rivals for treasure, fight
//! monster guards, or ambush a rival.

use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path;
use knight_engine::hex::{Hex, HexMap, Rng};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{self, Actor, Anim, Decor, MapGen};

const YOU: usize = 0;
const NAMES: [&str; 4] = ["You", "Morgana", "Brutus", "Seraphine"];
const COLORS: [u32; 4] = [0xffe27a, 0xff8a7a, 0xffb060, 0xe0a0ff];
/// Movement points per normal step (grass 2, road 1, forest 4, ...).
const STEP: u32 = 2;

struct Hero {
    actor: Actor,
    home: Hex,
    budget: ActionBudget,
    gold: u32,
    /// Knocked out until the next day.
    down: bool,
    /// Step already paid for (so the ongoing step keeps its terrain speed).
    paid: Option<(Hex, Hex)>,
    /// Attack this target when adjacent.
    pending: Option<Target>,
    think: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Hero(usize),
    Monster(usize),
}

struct Monster {
    actor: Actor,
    hex: Hex,
}

struct Floater {
    pos: Vec3,
    text: String,
    color: Color,
    t: f32,
}

pub struct HybridDemo {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    camera: Camera,
    controller: CameraController,
    clock: DayClock,
    heroes: Vec<Hero>,
    monsters: Vec<Monster>,
    treasure: HexMap<u32>,
    res: Reservations,
    rng: Rng,
    land: Vec<Hex>,
    hover: Option<Pick>,
    preview: Vec<Hex>,
    floaters: Vec<Floater>,
    fx: Particles,
    moves_per_day: u32,
    actions_per_day: u32,
    log: Vec<String>,
    sounds: Vec<(SoundId, Vec3)>,
}

fn hero_id(i: usize) -> u32 {
    i as u32 + 1
}

fn monster_id(k: usize) -> u32 {
    100 + k as u32
}

impl HybridDemo {
    pub fn new(art: Rc<Art>) -> Self {
        Self::with_day(art, 60.0)
    }

    pub fn with_day(art: Rc<Art>, day_length: f32) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 31, size: 16, amplitude: 6.0, island: 0.6, trees: 0.4, ..Default::default() },
        );
        let mut land: Vec<Hex> =
            world.hexes().filter(|&h| world.tile(h).is_some_and(|t| (1..=4).contains(&t.height))).collect();
        land.sort();
        let mut rng = Rng::new(21);
        let mut res = Reservations::new();
        // Four homes in the four quadrants.
        let homes: Vec<Hex> = [Vec2::new(-1.0, 1.0), Vec2::new(1.0, -1.0), Vec2::new(1.0, 1.0), Vec2::new(-1.0, -1.0)]
            .iter()
            .map(|d| {
                *land
                    .iter()
                    .max_by_key(|h| {
                        let p = world.layout.hex_to_point(**h);
                        ((p.x * d.x + p.y * d.y) * 10.0) as i64 - h.length() as i64 * 2
                    })
                    .unwrap()
            })
            .collect();
        let kinds = [UnitKind::Paladin, UnitKind::Knight, UnitKind::Ogre, UnitKind::Griffon];
        let heroes: Vec<Hero> = (0..4)
            .map(|i| {
                res.reserve(homes[i], hero_id(i));
                let team = if i == YOU { Team::Blue } else { Team::Red };
                let mut actor = Actor::new(kinds[i], team, &world, homes[i]);
                actor.hp = 60.0;
                actor.max_hp = 60.0;
                actor.mover.speed = 2.2;
                Hero {
                    actor,
                    home: homes[i],
                    budget: ActionBudget::new(16, 1),
                    gold: 0,
                    down: false,
                    paid: None,
                    pending: None,
                    think: 0.3 * i as f32,
                }
            })
            .collect();
        let mut decor = decor;
        decor.retain(|d| homes.iter().all(|h| h.distance(d.hex) > 1));
        let mut demo = HybridDemo {
            art,
            world,
            decor,
            camera: Camera::new(Vec3::ZERO, 34.0).with_pitch(52.0),
            controller: CameraController::default().drag_with(&[MouseButton::Right, MouseButton::Middle]),
            clock: DayClock::new(day_length),
            heroes,
            monsters: Vec::new(),
            treasure: HexMap::new(),
            res,
            rng: Rng::new(5),
            land,
            hover: None,
            preview: Vec::new(),
            floaters: Vec::new(),
            fx: Particles::new(),
            moves_per_day: 16,
            actions_per_day: 1,
            log: vec!["Day 1 begins.".into()],
            sounds: Vec::new(),
        };
        let _ = &mut rng;
        for _ in 0..10 {
            demo.drop_treasure();
        }
        for _ in 0..5 {
            demo.spawn_monster();
        }
        demo.camera.target = demo.world.hex_to_world(demo.heroes[YOU].home);
        demo
    }

    fn free(&self, h: Hex) -> bool {
        self.res.owner(h).is_none() && !self.treasure.contains(h)
    }

    fn drop_treasure(&mut self) {
        for _ in 0..40 {
            let h = self.land[self.rng.range(0, self.land.len() as i32) as usize];
            if self.free(h) && self.heroes.iter().all(|x| x.home.distance(h) > 2) {
                let amount = [100, 150, 250, 500][self.rng.range(0, 4) as usize];
                self.treasure.insert(h, amount);
                return;
            }
        }
    }

    fn spawn_monster(&mut self) {
        for _ in 0..40 {
            let h = self.land[self.rng.range(0, self.land.len() as i32) as usize];
            let k = self.monsters.len();
            if self.free(h) && self.heroes.iter().all(|x| x.home.distance(h) > 3) && self.res.reserve(h, monster_id(k))
            {
                let mut actor = Actor::new(UnitKind::Wolf, Team::Red, &self.world, h);
                actor.hp = 24.0;
                actor.max_hp = 24.0;
                actor.scale = 0.85;
                self.monsters.push(Monster { actor, hex: h });
                // Monsters guard gold.
                if let Some(n) = h.neighbors().into_iter().find(|&n| self.free(n) && self.world.move_cost(n).is_some())
                {
                    self.treasure.insert(n, 400);
                }
                return;
            }
        }
    }

    fn target_hex(&self, t: Target) -> Option<Hex> {
        match t {
            Target::Hero(j) => (!self.heroes[j].down).then(|| self.heroes[j].actor.hex()),
            Target::Monster(k) => self.monsters[k].actor.alive().then_some(self.monsters[k].hex),
        }
    }

    fn target_at(&self, h: Hex, me: usize) -> Option<Target> {
        let o = self.res.owner(h)?;
        if o >= 100 {
            let k = (o - 100) as usize;
            return self.monsters[k].actor.alive().then_some(Target::Monster(k));
        }
        let j = o as usize - 1;
        (j != me && !self.heroes[j].down).then_some(Target::Hero(j))
    }

    /// Route for hero `i` to `goal`, truncated to what today's movement points pay for.
    fn plan(&self, i: usize, goal: Hex) -> (Vec<Hex>, Vec<Hex>) {
        let h = &self.heroes[i];
        let from = h.actor.mover.next_stop();
        let me = hero_id(i);
        let Some(p) = path::astar(from, goal, 1, 200, |a, b| {
            let c = self.world.step_points(a, b, 1, STEP)?;
            if b != goal && !self.res.is_free(b, me) {
                return None;
            }
            Some(c)
        }) else {
            return (Vec::new(), Vec::new());
        };
        let mut hexes = p.hexes;
        // Never step onto an occupied goal: stop next to it.
        if hexes.len() > 1 && !self.res.is_free(*hexes.last().unwrap(), me) {
            hexes.pop();
        }
        let mut left = h.budget.moves;
        let mut cut = hexes.len();
        for k in 1..hexes.len() {
            let c = self.world.step_points(hexes[k - 1], hexes[k], 1, STEP).unwrap_or(99);
            if c > left {
                cut = k;
                break;
            }
            left -= c;
        }
        let beyond = hexes[cut.saturating_sub(1)..].to_vec();
        hexes.truncate(cut);
        (hexes, if beyond.len() > 1 { beyond } else { Vec::new() })
    }

    fn order(&mut self, i: usize, goal: Hex) {
        let target = self.target_at(goal, i);
        let (route, _) = self.plan(i, goal);
        let h = &mut self.heroes[i];
        h.pending = target;
        if route.len() > 1 {
            h.actor.walk_hexes(&route);
        } else {
            h.actor.mover.stop();
        }
    }

    fn say(&mut self, s: String) {
        self.log.push(s);
        if self.log.len() > 6 {
            self.log.remove(0);
        }
    }

    fn float(&mut self, pos: Vec3, text: String, color: Color) {
        self.floaters.push(Floater { pos: pos + Vec3::Y * 1.6, text, color, t: 0.0 });
    }

    fn attack(&mut self, i: usize, t: Target) {
        if !self.heroes[i].budget.spend_action() {
            if i == YOU {
                self.sounds.push((self.art.sfx.deny, self.heroes[i].actor.pos));
            }
            return;
        }
        let dmg = self.rng.range(9, 15) as f32;
        let from = self.heroes[i].actor.pos;
        self.sounds.push((self.art.sfx.swing, from));
        self.sounds.push((self.art.sfx.hit, from));
        let name = NAMES[i];
        self.heroes[i].actor.play(Anim::Attack);
        match t {
            Target::Monster(k) => {
                let m = &mut self.monsters[k];
                self.heroes[i].actor.facing_left = m.actor.pos.x < from.x;
                m.actor.hp -= dmg;
                let pos = m.actor.pos;
                self.float(pos, format!("-{dmg:.0}"), Color::WHITE);
                if self.monsters[k].actor.alive() {
                    self.monsters[k].actor.play(Anim::Hurt);
                    // The wolf bites back.
                    let bite = self.rng.range(4, 9) as f32;
                    self.heroes[i].actor.hp -= bite;
                    self.float(from, format!("-{bite:.0}"), Color::hex(0xff7a6a));
                } else {
                    self.monsters[k].actor.play(Anim::Death);
                    self.res.release_all(monster_id(k));
                    self.heroes[i].gold += 150;
                    self.float(pos, "+150 gold".into(), Color::YELLOW);
                    self.say(format!("{name} slew a wolf (+150)."));
                }
            }
            Target::Hero(j) => {
                let pos = self.heroes[j].actor.pos;
                self.heroes[i].actor.facing_left = pos.x < from.x;
                self.heroes[j].actor.hp -= dmg;
                self.float(pos, format!("-{dmg:.0}"), Color::hex(0xff7a6a));
                self.heroes[j].actor.play(Anim::Hurt);
                self.say(format!("{name} attacked {}!", NAMES[j]));
            }
        }
        if self.heroes[i].actor.hp <= 0.0 {
            self.knock_out(i, None);
        }
        if let Target::Hero(j) = t
            && self.heroes[j].actor.hp <= 0.0
        {
            self.knock_out(j, Some(i));
        }
    }

    fn knock_out(&mut self, j: usize, by: Option<usize>) {
        let h = &mut self.heroes[j];
        if h.down {
            return;
        }
        h.down = true;
        h.actor.hp = 0.0;
        h.actor.mover.stop();
        h.budget.moves = 0;
        h.budget.actions = 0;
        let loot = h.gold / 4;
        h.gold -= loot;
        if let Some(i) = by {
            self.heroes[i].gold += loot;
            self.say(format!("{} knocked out {} and took {loot} gold!", NAMES[i], NAMES[j]));
        } else {
            self.say(format!("{} was knocked out.", NAMES[j]));
        }
    }

    fn dawn(&mut self) {
        let day = self.clock.day;
        for i in 0..self.heroes.len() {
            let (mpd, apd) = (self.moves_per_day, self.actions_per_day);
            let h = &mut self.heroes[i];
            h.budget = ActionBudget::new(mpd, apd);
            h.pending = None;
            if h.down {
                // Back home, patched up.
                self.res.release_all(hero_id(i));
                let home = self.heroes[i].home;
                let spot = std::iter::once(home)
                    .chain(home.spiral(2))
                    .find(|&x| self.res.is_free(x, hero_id(i)))
                    .unwrap_or(home);
                self.res.reserve(spot, hero_id(i));
                let h = &mut self.heroes[i];
                h.actor.place(&self.world, spot);
                h.actor.hp = h.actor.max_hp * 0.5;
                h.actor.play(Anim::Idle);
                h.down = false;
            } else {
                h.actor.hp = (h.actor.hp + 10.0).min(h.actor.max_hp);
            }
        }
        for _ in 0..3 {
            self.drop_treasure();
        }
        if day.is_multiple_of(2) {
            self.spawn_monster();
        }
        self.say(format!("Day {day} begins."));
    }

    /// Rivals: head for the most valuable nearby treasure; hit whoever is adjacent (monsters
    /// first) when they have an action; declare ready when done.
    fn think(&mut self, i: usize) {
        if self.heroes[i].down || self.heroes[i].actor.moving() {
            return;
        }
        let me = self.heroes[i].actor.hex();
        if self.heroes[i].budget.actions > 0 {
            let foe = me.neighbors().into_iter().filter_map(|n| self.target_at(n, i)).min_by_key(|t| match t {
                Target::Monster(_) => 0,
                Target::Hero(_) => 1,
            });
            if let Some(t) = foe
                && (matches!(t, Target::Monster(_)) || self.rng.chance(0.5))
            {
                self.attack(i, t);
                return;
            }
        }
        if self.heroes[i].budget.moves >= STEP {
            let best = self
                .treasure
                .iter()
                .map(|(h, &g)| (h, g as f32 / (1.0 + me.distance(h) as f32)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(h, _)| h);
            if let Some(goal) = best {
                let (route, _) = self.plan(i, goal);
                if route.len() > 1 {
                    self.heroes[i].actor.walk_hexes(&route);
                    return;
                }
            }
        }
        self.clock.set_ready(i as u64, true);
    }

    fn step_heroes(&mut self, dt: f32) {
        for i in 0..self.heroes.len() {
            let world = &self.world;
            let (res, hero) = (&mut self.res, &mut self.heroes[i]);
            let me = hero_id(i);
            let Hero { actor, budget, paid, .. } = hero;
            let paid_now = *paid;
            let events = actor.update_with(world, &self.art, dt, |a, b| {
                let cost = world.step_points(a, b, 1, STEP)?;
                // The step in progress is paid; a new one needs enough points and a free hex.
                if paid_now != Some((a, b)) && budget.moves < cost {
                    return None;
                }
                res.reserve(b, me).then(|| world.step_cost(a, b, 1).unwrap_or(1.0))
            });
            res.track(me, &events);
            for e in &events {
                match *e {
                    Step::Left(a, b) => {
                        if i == YOU {
                            self.sounds.push((self.art.sfx.step, world.hex_to_world(b)));
                        }
                        let cost = world.step_points(a, b, 1, STEP).unwrap_or(0);
                        budget.spend_moves(cost);
                        *paid = Some((a, b));
                    }
                    Step::Blocked(_) if actor.mover.blocked_for > 0.3 => actor.mover.stop(),
                    _ => {}
                }
            }
            let arrived: Vec<Hex> =
                events.iter().filter_map(|e| if let Step::Arrived(h) = e { Some(*h) } else { None }).collect();
            for h in arrived {
                if let Some(g) = self.treasure.remove(h) {
                    self.heroes[i].gold += g;
                    let pos = self.world.hex_to_world(h);
                    self.sounds.push((self.art.sfx.coin, pos));
                    self.float(pos, format!("+{g} gold"), Color::YELLOW);
                    self.fx.burst(pos + Vec3::Y * 0.5, &Burst { count: 24, ..Burst::sparks(Color::hex(0xffd84a)) });
                }
            }
            // Pending attack once adjacent and standing.
            if let Some(t) = self.heroes[i].pending
                && self.heroes[i].actor.mover.step().is_none()
            {
                match self.target_hex(t) {
                    Some(th) if th.distance(self.heroes[i].actor.hex()) == 1 => {
                        self.heroes[i].pending = None;
                        self.heroes[i].actor.mover.stop();
                        self.attack(i, t);
                    }
                    Some(_) if self.heroes[i].actor.moving() => {}
                    _ => self.heroes[i].pending = None,
                }
            }
        }
        for m in &mut self.monsters {
            m.actor.anim_t += dt;
            if !m.actor.alive() {
                m.actor.play(Anim::Death);
            } else if matches!(m.actor.anim, Anim::Hurt) && m.actor.anim_t > 0.4 {
                m.actor.play(Anim::Idle);
            }
        }
    }
}

impl Scene for HybridDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let dt = ctx.time.dt;
        let input = &ctx.input;
        for (id, len) in [("day20", 20.0), ("day40", 40.0), ("day60", 60.0), ("day120", 120.0)] {
            if input.ui_clicked(id) {
                self.clock.day_length = len;
            }
        }
        for (id, m) in [("mv8", 8), ("mv16", 16), ("mv24", 24)] {
            if input.ui_clicked(id) {
                self.moves_per_day = m;
            }
        }
        for (id, a) in [("act1", 1), ("act2", 2), ("act3", 3)] {
            if input.ui_clicked(id) {
                self.actions_per_day = a;
            }
        }
        if input.key_pressed(Key::Enter) || input.ui_clicked("ready") {
            let r = !self.clock.is_ready(YOU as u64);
            self.clock.set_ready(YOU as u64, r);
        }
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        self.hover = if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)) };
        let you_can_act = !self.heroes[YOU].down;
        self.preview = match self.hover {
            Some(p) if you_can_act => {
                let (ok, beyond) = self.plan(YOU, p.hex);
                let mut v = ok;
                if !beyond.is_empty() {
                    v.extend(beyond.into_iter().skip(1));
                }
                v
            }
            _ => Vec::new(),
        };
        if ctx.input.clicked(MouseButton::Left)
            && !over_ui
            && you_can_act
            && let Some(p) = self.hover
        {
            let near = self.heroes[YOU].actor.hex().distance(p.hex) == 1;
            match self.target_at(p.hex, YOU) {
                Some(t) if near && self.heroes[YOU].actor.mover.step().is_none() => self.attack(YOU, t),
                _ => self.order(YOU, p.hex),
            }
        }

        // Rivals think a few times per second.
        for i in 1..self.heroes.len() {
            self.heroes[i].think -= dt;
            if self.heroes[i].think <= 0.0 {
                self.heroes[i].think = 0.6;
                self.think(i);
            }
        }
        // You are ready automatically once nothing is left to do.
        if self.heroes[YOU].budget.exhausted() || self.heroes[YOU].down {
            self.clock.set_ready(YOU as u64, true);
        }
        self.step_heroes(dt);
        common::flush_sounds(ctx, &mut self.sounds, &self.camera);
        let everyone: Vec<u64> = (0..self.heroes.len() as u64).collect();
        if self.clock.update(dt, &everyone).is_some() {
            ctx.audio.play(self.art.sfx.powerup);
            self.dawn();
        }
        for f in &mut self.floaters {
            f.t += dt;
            f.pos.y += dt * 0.7;
        }
        self.floaters.retain(|f| f.t < 1.5);
        self.fx.update(dt);
        let h = &self.heroes[YOU];
        ctx.status = format!(
            "hybrid day={} left={:.0} moves={} actions={} gold={} rivals={} ready={} treasure={}",
            self.clock.day,
            self.clock.time_left(),
            h.budget.moves,
            h.budget.actions,
            h.gold,
            self.heroes[1..].iter().map(|x| x.gold.to_string()).collect::<Vec<_>>().join("/"),
            self.clock.ready_count(),
            self.treasure.len()
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        // The sun crosses the sky once per day: warm dawn, bright noon, orange dusk.
        let k = self.clock.progress();
        let ambient = if k < 0.15 {
            Color::hex(0xffc0a0).lerp(Color::WHITE, k / 0.15)
        } else if k > 0.8 {
            Color::WHITE.lerp(Color::hex(0xd08070), (k - 0.8) / 0.2)
        } else {
            Color::WHITE
        };
        let budget_moves = self.heroes[YOU].budget.moves;
        {
            let mut w = f.world(&mut self.world, &self.camera);
            w.set_ambient(ambient);
            for d in &self.decor {
                d.draw(&mut w);
            }
            for (i, h) in self.heroes.iter().enumerate() {
                let p = w.world.hex_to_world(h.home);
                w.sprite(
                    Sprite::new(self.art.flag[if i == YOU { 0 } else { 1 }], p + Vec3::new(0.5, 0.0, -0.1)).scale(0.8),
                );
            }
            for (h, _) in self.treasure.iter() {
                w.sprite(Sprite::new(self.art.treasure, w.world.hex_to_world(h)).scale(0.8));
            }
            // Your route: affordable today in green, the rest (tomorrow) in red.
            w.set_on_top(true);
            let mut left = budget_moves as i64;
            for pair in self.preview.windows(2) {
                left -= w.world.step_points(pair[0], pair[1], 1, STEP).unwrap_or(99) as i64;
                let col = if left >= 0 { Color::hex(0x6ef08a) } else { Color::hex(0xff6a5a) };
                let (a, b) = (w.world.hex_to_world(pair[0]), w.world.hex_to_world(pair[1]));
                w.line(a, b, 0.08, col.with_alpha(0.8));
                w.ring(b, 0.2, col);
            }
            if let Some(p) = self.hover {
                w.hex_outline(p.hex, Color::WHITE.with_alpha(0.6), 0.07);
            }
            w.set_on_top(false);
            for m in &self.monsters {
                m.actor.draw(&mut w, &self.art, true);
            }
            for (i, h) in self.heroes.iter().enumerate() {
                let mut a = h.actor.clone();
                a.selected = i == YOU;
                if h.down {
                    a.tint = Color::rgba(0.5, 0.5, 0.5, 0.5);
                }
                a.draw(&mut w, &self.art, true);
                let ready = self.clock.is_ready(i as u64);
                let label = if h.down {
                    format!("{} (out)", NAMES[i])
                } else if ready {
                    format!("{} - ready", NAMES[i])
                } else {
                    NAMES[i].to_string()
                };
                w.label(h.actor.pos + Vec3::Z * 0.55, &label, 8.0, Color::hex(COLORS[i]));
            }
            for fl in &self.floaters {
                w.label(fl.pos, &fl.text, 16.0, fl.color.with_alpha((1.5 - fl.t).clamp(0.0, 1.0)));
            }
            self.fx.draw(&mut w);
        }
        common::header(f, "Hybrid: Timed Days", "Everyone acts at once; each day brings a budget of moves and actions");
        common::stats(f);

        let size = f.ui_size();
        let t = f.theme.clone();
        // Day bar: the timer is the turn.
        let bar = Rect::new(size.x * 0.5 - 220.0, 70.0, 440.0, 46.0);
        f.panel(bar);
        let left = self.clock.time_left();
        f.text(Vec2::new(bar.x + 10.0, bar.y + 8.0), &format!("DAY {}", self.clock.day), 16.0, t.accent);
        f.text(
            Vec2::new(bar.x + 110.0, bar.y + 10.0),
            &format!("{:.0}s left   ready {}/4", left.ceil(), self.clock.ready_count()),
            8.0,
            t.text,
        );
        f.bar(
            Rect::new(bar.x + 10.0, bar.y + 30.0, bar.w - 20.0, 8.0),
            1.0 - self.clock.progress(),
            Color::hex(0xf0c65a),
            Color::hex(0x201a10),
        );

        // Your budget.
        let you = &self.heroes[YOU];
        let r = Rect::new(size.x - 448.0, size.y - 92.0, 440.0, 84.0);
        f.panel(r);
        f.text(Vec2::new(r.x + 12.0, r.y + 10.0), "TODAY", 8.0, t.text_dim);
        f.text(
            Vec2::new(r.x + 12.0, r.y + 26.0),
            &format!("Moves {} / {}", you.budget.moves, you.budget.moves_per_day),
            8.0,
            t.text,
        );
        f.bar(
            Rect::new(r.x + 12.0, r.y + 40.0, 160.0, 8.0),
            you.budget.moves as f32 / you.budget.moves_per_day.max(1) as f32,
            Color::hex(0x6ef08a),
            Color::hex(0x102010),
        );
        f.text(
            Vec2::new(r.x + 12.0, r.y + 56.0),
            &format!("Actions {} / {}   Gold {}", you.budget.actions, you.budget.actions_per_day, you.gold),
            8.0,
            t.text,
        );
        let status = if you.down {
            "Knocked out - back at dawn".to_string()
        } else if you.budget.exhausted() {
            format!("Done for today - dawn in {:.0}s", left.ceil())
        } else if self.clock.is_ready(YOU as u64) {
            "Ready (Enter to un-ready)".to_string()
        } else {
            "Act now, or press Ready".to_string()
        };
        f.text(Vec2::new(r.x + 190.0, r.y + 10.0), &status, 8.0, t.accent);
        let ready = self.clock.is_ready(YOU as u64);
        f.button_ex(
            "ready",
            Rect::new(r.x + 190.0, r.y + 30.0, 238.0, 42.0),
            if ready { "Ready!" } else { "Ready [Enter]" },
            true,
            ready,
        );

        // Leaderboard + settings.
        let lb = Rect::new(size.x - 208.0, 106.0, 200.0, 250.0);
        f.panel(lb);
        f.text(Vec2::new(lb.x + 10.0, lb.y + 8.0), "GOLD", 8.0, t.text_dim);
        let mut order: Vec<usize> = (0..self.heroes.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.heroes[i].gold));
        for (row, &i) in order.iter().enumerate() {
            let y = lb.y + 22.0 + row as f32 * 14.0;
            f.text(Vec2::new(lb.x + 10.0, y), NAMES[i], 8.0, Color::hex(COLORS[i]));
            f.text(Vec2::new(lb.x + 130.0, y), &format!("{}", self.heroes[i].gold), 8.0, t.text);
        }
        let dim = t.text_dim;
        f.text(Vec2::new(lb.x + 10.0, lb.y + 86.0), "DAY LENGTH (s)", 8.0, dim);
        let c = Rect::new(lb.x + 8.0, lb.y + 98.0, lb.w - 16.0, 22.0).cols(4, 3.0);
        for (k, (id, len)) in
            [("day20", 20.0), ("day40", 40.0), ("day60", 60.0), ("day120", 120.0)].into_iter().enumerate()
        {
            f.button_ex(id, c[k], &format!("{len:.0}"), true, self.clock.day_length == len);
        }
        f.text(Vec2::new(lb.x + 10.0, lb.y + 128.0), "MOVES / DAY", 8.0, dim);
        let c = Rect::new(lb.x + 8.0, lb.y + 140.0, lb.w - 16.0, 22.0).cols(3, 3.0);
        for (k, (id, m)) in [("mv8", 8), ("mv16", 16), ("mv24", 24)].into_iter().enumerate() {
            f.button_ex(id, c[k], &format!("{m}"), true, self.moves_per_day == m);
        }
        f.text(Vec2::new(lb.x + 10.0, lb.y + 170.0), "ACTIONS / DAY", 8.0, dim);
        let c = Rect::new(lb.x + 8.0, lb.y + 182.0, lb.w - 16.0, 22.0).cols(3, 3.0);
        for (k, (id, a)) in [("act1", 1), ("act2", 2), ("act3", 3)].into_iter().enumerate() {
            f.button_ex(id, c[k], &format!("{a}"), true, self.actions_per_day == a);
        }
        f.text(Vec2::new(lb.x + 10.0, lb.y + 212.0), "(applies from next day)", 8.0, dim);

        // Event log.
        let lg = Rect::new(8.0, size.y - 184.0, 330.0, 92.0);
        f.panel(lg);
        for (k, l) in self.log.iter().rev().take(6).enumerate() {
            f.text(
                Vec2::new(lg.x + 8.0, lg.y + 8.0 + k as f32 * 13.0),
                l,
                8.0,
                t.text.with_alpha(1.0 - k as f32 * 0.14),
            );
        }
        common::help(
            f,
            &[
                "Click a hex: walk (green = affordable today)   Click an adjacent foe: attack",
                "Moves cost terrain points; 1 action per attack   Enter: ready   Right drag: pan",
            ],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Days pass on the timer; rivals spend their budgets gathering gold.
    #[test]
    fn days_pass_and_rivals_collect() {
        let mut ctx = Context::new();
        let art = Art::load(&mut ctx);
        let mut d = HybridDemo::with_day(art, 8.0);
        ctx.time.dt = 1.0 / 30.0;
        for _ in 0..(30 * 40) {
            let _ = d.update(&mut ctx);
            ctx.input.end_frame();
        }
        assert!(d.clock.day >= 5, "day {}", d.clock.day);
        let rival_gold: u32 = d.heroes[1..].iter().map(|h| h.gold).sum();
        assert!(rival_gold > 0, "rivals collected gold");
        // Nobody ever overspent.
        assert!(d.heroes.iter().all(|h| h.budget.moves <= h.budget.moves_per_day));
    }
}
