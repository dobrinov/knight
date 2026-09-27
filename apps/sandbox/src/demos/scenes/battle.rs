//! Hex battle: stacks take turns by speed, move within their range, attack (with retaliation) or
//! shoot. The enemy side (and optionally yours, with Auto) is played by a small AI.

use std::collections::VecDeque;
use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path;
use knight_engine::hex::{Hex, HexMap, Offset, Parity, Rng, shapes};
use knight_engine::*;

use super::{BattleResult, Stack, stats};
use crate::art::{Art, Team};
use crate::common::{self, Actor, Anim};

const COLS: i32 = 13;
const ROWS: i32 = 9;
/// Movement points per normal step.
const POINTS: u32 = 10;

struct BUnit {
    stack: Stack,
    team: Team,
    hex: Hex,
    actor: Actor,
    hp_top: u32,
    retaliated: bool,
    start_count: u32,
}

impl BUnit {
    fn alive(&self) -> bool {
        self.stack.count > 0
    }
}

#[derive(Clone, Debug)]
enum Command {
    Move(Vec<Hex>),
    MoveAttack(Vec<Hex>, usize),
    Shoot(usize),
    Wait,
}

enum Phase {
    Input,
    Moving { then: Option<usize> },
    Striking { attacker: usize, target: usize, ranged: bool, retaliation: bool, t: f32, dealt: bool },
    Pause(f32),
    Over,
}

struct Floater {
    pos: Vec3,
    text: String,
    color: Color,
    t: f32,
}

pub struct BattleScene {
    art: Rc<Art>,
    world: HexWorld,
    camera: Camera,
    controller: CameraController,
    units: Vec<BUnit>,
    order: VecDeque<usize>,
    current: usize,
    round: u32,
    phase: Phase,
    reach: path::Reachable,
    hover: Option<Pick>,
    preview: Option<Command>,
    rng: Rng,
    floaters: Vec<Floater>,
    log: VecDeque<String>,
    auto: bool,
    won: Option<bool>,
    sounds: Vec<(SoundId, Vec3)>,
}

fn cell(col: i32, row: i32) -> Hex {
    Offset::new(col, row).to_hex_r(Parity::Odd)
}

impl BattleScene {
    pub fn new(art: Rc<Art>, army: Vec<Stack>, enemies: Vec<Stack>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        world.water = None;
        world.base_height = -2;
        let mut rng = Rng::new(army.iter().map(|s| s.count as u64).sum::<u64>() * 31 + enemies.len() as u64);
        for h in shapes::rectangle(COLS, ROWS, true, Parity::Odd) {
            let mat = if rng.chance(0.15) { mats.meadow } else { mats.grass };
            world.set_tile(h, Tile::new(0, mat));
        }
        // A low hill and a few rocks in the middle columns.
        let hill = cell(6, rng.range(2, 7));
        for h in hill.spiral(1) {
            if world.contains(h) {
                world.set_tile(h, Tile::new(1, mats.dirt));
            }
        }
        for _ in 0..4 {
            let h = cell(rng.range(3, 10), rng.range(0, ROWS));
            if h.distance(hill) > 1 {
                world.set_tile(h, Tile::new(2, mats.rock));
            }
        }

        let mut units = Vec::new();
        let rows = [4, 2, 6, 0, 8, 1, 7];
        for (i, s) in army.iter().enumerate() {
            let h = cell(0, rows[i % rows.len()]);
            units.push(Self::make_unit(&art, &world, *s, Team::Blue, h));
        }
        // Neutral stacks split like in Heroes.
        let mut split = Vec::new();
        for s in enemies {
            let parts = if s.count >= 6 {
                3
            } else if s.count >= 2 {
                2
            } else {
                1
            };
            for p in 0..parts {
                let n = s.count / parts + if p < s.count % parts { 1 } else { 0 };
                split.push(Stack::new(s.kind, n));
            }
        }
        for (i, s) in split.iter().enumerate() {
            let h = cell(COLS - 1, rows[i % rows.len()]);
            units.push(Self::make_unit(&art, &world, *s, Team::Red, h));
        }
        let center = world.center();
        let camera = Camera::new(center + Vec3::new(0.0, 0.0, 0.6), 50.0).with_pitch(52.0);
        let mut b = BattleScene {
            art,
            world,
            camera,
            controller: CameraController::rts(),
            units,
            order: VecDeque::new(),
            current: 0,
            round: 0,
            phase: Phase::Input,
            reach: path::Reachable::default(),
            hover: None,
            preview: None,
            rng,
            floaters: Vec::new(),
            log: VecDeque::new(),
            auto: false,
            won: None,
            sounds: Vec::new(),
        };
        b.next_turn();
        b
    }

    fn make_unit(art: &Art, world: &HexWorld, stack: Stack, team: Team, hex: Hex) -> BUnit {
        let mut actor = Actor::new(stack.kind, team, world, hex);
        actor.mover.speed = 4.5;
        actor.flying = stats(stack.kind).flying;
        let _ = art;
        BUnit { stack, team, hex, actor, hp_top: stats(stack.kind).hp, retaliated: false, start_count: stack.count }
    }

    fn occupied(&self) -> HexMap<usize> {
        self.units.iter().enumerate().filter(|(_, u)| u.alive()).map(|(i, u)| (u.hex, i)).collect()
    }

    fn compute_reach(&mut self) {
        let u = &self.units[self.current];
        let s = stats(u.stack.kind);
        let occ = self.occupied();
        let me = u.hex;
        let world = &self.world;
        let reach = if s.flying {
            // Fliers ignore obstacles on the way; only the landing hex matters.
            let mut r = path::reachable(me, s.speed as u32, |_, b| world.contains(b).then_some(1));
            r.nodes.retain(|h, _| h == me || (!occ.contains(h) && world.tile(h).is_some_and(|t| t.height < 2)));
            r
        } else {
            // Speed is in normal steps; terrain makes each step cheaper or dearer.
            path::reachable(me, s.speed as u32 * POINTS, |a, b| {
                if occ.contains(b) || world.tile(b)?.height >= 2 {
                    return None;
                }
                world.step_points(a, b, 1, POINTS)
            })
        };
        self.reach = reach;
    }

    fn path_to(&self, h: Hex) -> Option<Vec<Hex>> {
        let s = stats(self.units[self.current].stack.kind);
        if s.flying {
            // Fliers still travel hex by hex, straight over whatever is below.
            return self.reach.contains(h).then(|| self.units[self.current].hex.line_to(h));
        }
        self.reach.path_to(h)
    }

    fn next_turn(&mut self) {
        let alive = |t: Team| self.units.iter().any(|u| u.team == t && u.alive());
        if !alive(Team::Blue) || !alive(Team::Red) {
            self.won = Some(alive(Team::Blue));
            self.phase = Phase::Over;
            let msg = if self.won == Some(true) { "Victory!" } else { "Defeat..." };
            self.say(msg.to_string());
            return;
        }
        loop {
            if self.order.is_empty() {
                self.round += 1;
                let mut idx: Vec<usize> = (0..self.units.len()).filter(|&i| self.units[i].alive()).collect();
                idx.sort_by_key(|&i| (-stats(self.units[i].stack.kind).speed, self.units[i].team == Team::Red));
                self.order = idx.into();
                for u in &mut self.units {
                    u.retaliated = false;
                }
                self.say(format!("Round {}", self.round));
            }
            let i = self.order.pop_front().unwrap();
            if self.units[i].alive() {
                self.current = i;
                break;
            }
        }
        self.compute_reach();
        self.phase = Phase::Input;
        self.preview = None;
    }

    fn say(&mut self, s: String) {
        self.log.push_front(s);
        self.log.truncate(5);
    }

    /// The command for clicking `pick` with the current unit.
    fn command_for(&self, pick: Pick) -> Option<Command> {
        let occ = self.occupied();
        let cur = &self.units[self.current];
        let s = stats(cur.stack.kind);
        if let Some(&t) = occ.get(pick.hex) {
            if self.units[t].team == cur.team {
                return None;
            }
            let adjacent = cur.hex.distance(pick.hex) == 1;
            if s.ranged && !adjacent {
                return Some(Command::Shoot(t));
            }
            // Attack from the free neighbour of the target closest to where the cursor points.
            let mut best: Option<(f32, Vec<Hex>)> = None;
            for n in pick.hex.neighbors() {
                let path = if n == cur.hex {
                    Some(vec![cur.hex])
                } else if occ.contains(n) {
                    None
                } else {
                    self.path_to(n)
                };
                if let Some(p) = path {
                    let c = self.world.hex_to_world(n);
                    let d = Vec2::new(c.x - pick.point.x, c.z - pick.point.z).length();
                    if best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                        best = Some((d, p));
                    }
                }
            }
            return best.map(|(_, p)| Command::MoveAttack(p, t));
        }
        if pick.hex != cur.hex && self.reach.contains(pick.hex) {
            return self.path_to(pick.hex).map(Command::Move);
        }
        None
    }

    fn ai_command(&self) -> Command {
        let cur = &self.units[self.current];
        let s = stats(cur.stack.kind);
        let foes: Vec<usize> =
            (0..self.units.len()).filter(|&i| self.units[i].alive() && self.units[i].team != cur.team).collect();
        let adjacent_foe = foes.iter().any(|&f| self.units[f].hex.distance(cur.hex) == 1);
        if s.ranged && !adjacent_foe {
            let weakest = foes
                .iter()
                .copied()
                .min_by_key(|&f| self.units[f].stack.count * stats(self.units[f].stack.kind).hp)
                .unwrap();
            return Command::Shoot(weakest);
        }
        // Best melee option this turn: the foe we can hit that we damage most relative to its size.
        let mut best: Option<(u32, Command)> = None;
        for &f in &foes {
            let fh = self.units[f].hex;
            let p = self.world.hex_to_world(fh);
            if let Some(Command::MoveAttack(path, t)) = self.command_for(Pick { hex: fh, point: p }) {
                let score = 10_000 - self.units[t].stack.count * stats(self.units[t].stack.kind).hp.min(9_999);
                if best.as_ref().is_none_or(|(b, _)| score > *b) {
                    best = Some((score, Command::MoveAttack(path, t)));
                }
            }
        }
        if let Some((_, c)) = best {
            return c;
        }
        // Otherwise close in on the nearest foe.
        let target = foes.iter().map(|&f| self.units[f].hex).min_by_key(|h| h.distance(cur.hex)).unwrap();
        let dest = self.reach.hexes().min_by_key(|h| (h.distance(target), h.q, h.r)).unwrap_or(cur.hex);
        if dest == cur.hex { Command::Wait } else { self.path_to(dest).map(Command::Move).unwrap_or(Command::Wait) }
    }

    fn execute(&mut self, cmd: Command) {
        let i = self.current;
        match cmd {
            Command::Move(p) => {
                self.move_unit(i, &p);
                self.phase = Phase::Moving { then: None };
            }
            Command::MoveAttack(p, t) => {
                self.move_unit(i, &p);
                self.phase = Phase::Moving { then: Some(t) };
            }
            Command::Shoot(t) => {
                self.face(i, t);
                self.units[i].actor.play(Anim::Shoot);
                self.phase =
                    Phase::Striking { attacker: i, target: t, ranged: true, retaliation: false, t: 0.0, dealt: false };
            }
            Command::Wait => {
                let name = self.units[i].stack.kind.name();
                self.say(format!("{name} defends."));
                self.phase = Phase::Pause(0.3);
            }
        }
    }

    fn move_unit(&mut self, i: usize, p: &[Hex]) {
        let last = *p.last().unwrap();
        let u = &mut self.units[i];
        u.actor.walk_hexes(p);
        u.hex = last;
    }

    fn face(&mut self, i: usize, t: usize) {
        let (a, b) = (self.units[i].actor.pos, self.units[t].actor.pos);
        self.units[i].actor.facing_left = b.x < a.x;
    }

    fn deal(&mut self, attacker: usize, target: usize, ranged: bool) {
        let a = &self.units[attacker];
        let s = stats(a.stack.kind);
        let roll = self.rng.range(s.min_dmg as i32, s.max_dmg as i32 + 1) as u32;
        let mut dmg = a.stack.count * roll;
        if ranged && a.hex.distance(self.units[target].hex) > 6 {
            dmg /= 2;
        }
        let an = a.stack.kind.name();
        let t = &mut self.units[target];
        let hp = stats(t.stack.kind).hp;
        let pool = (t.stack.count - 1) * hp + t.hp_top;
        let before = t.stack.count;
        if dmg >= pool {
            t.stack.count = 0;
        } else {
            let left = pool - dmg;
            t.stack.count = left.div_ceil(hp);
            t.hp_top = left - (t.stack.count - 1) * hp;
        }
        let killed = before - t.stack.count;
        t.actor.play(if t.alive() { Anim::Hurt } else { Anim::Death });
        if !t.alive() {
            t.actor.hp = 0.0;
        }
        let sfx = &self.art.sfx;
        let hit_at = t.actor.pos;
        self.sounds.push((if ranged { sfx.shoot } else { sfx.swing }, hit_at));
        self.sounds.push((sfx.hit, hit_at));
        let pos = t.actor.pos + Vec3::Y * 1.6;
        let tn = t.stack.kind.name();
        self.floaters.push(Floater { pos, text: format!("-{dmg}"), color: Color::hex(0xff7a6a), t: 0.0 });
        self.say(format!("{an} deal {dmg} damage, {killed} {tn} perish."));
    }

    fn step(&mut self, dt: f32) {
        for u in &mut self.units {
            u.actor.update(&self.world, &self.art, dt);
        }
        match &mut self.phase {
            Phase::Moving { then } => {
                let then = *then;
                if !self.units[self.current].actor.moving() {
                    match then {
                        Some(t) => {
                            let a = self.current;
                            self.face(a, t);
                            self.units[a].actor.play(Anim::Attack);
                            self.phase = Phase::Striking {
                                attacker: a,
                                target: t,
                                ranged: false,
                                retaliation: false,
                                t: 0.0,
                                dealt: false,
                            };
                        }
                        None => self.phase = Phase::Pause(0.15),
                    }
                }
            }
            Phase::Striking { attacker, target, ranged, retaliation, t, dealt } => {
                *t += dt;
                let (a, tg, r, ret) = (*attacker, *target, *ranged, *retaliation);
                if !*dealt && *t >= 0.25 {
                    *dealt = true;
                    self.deal(a, tg, r);
                }
                if let Phase::Striking { t, .. } = self.phase
                    && t >= 0.55
                {
                    let can_retaliate = !r && !ret && self.units[tg].alive() && !self.units[tg].retaliated;
                    if can_retaliate {
                        self.units[tg].retaliated = true;
                        self.face(tg, a);
                        self.units[tg].actor.play(Anim::Attack);
                        self.phase = Phase::Striking {
                            attacker: tg,
                            target: a,
                            ranged: false,
                            retaliation: true,
                            t: 0.0,
                            dealt: false,
                        };
                    } else {
                        self.phase = Phase::Pause(0.2);
                    }
                }
            }
            Phase::Pause(t) => {
                *t -= dt;
                if *t <= 0.0 {
                    self.next_turn();
                }
            }
            Phase::Input | Phase::Over => {}
        }
        for f in &mut self.floaters {
            f.t += dt;
            f.pos.y += dt * 0.8;
        }
        self.floaters.retain(|f| f.t < 1.2);
    }

    fn result(&self) -> BattleResult {
        let army =
            self.units.iter().filter(|u| u.team == Team::Blue && u.alive()).fold(Vec::<Stack>::new(), |mut acc, u| {
                super::add_to_army(&mut acc, u.stack);
                acc
            });
        BattleResult { won: self.won == Some(true), army }
    }
}

impl Scene for BattleScene {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if ctx.input.key_pressed(Key::P) || ctx.input.ui_clicked("pixel") {
            common::toggle_pixel_mode(ctx);
        }
        if ctx.input.key_pressed(Key::A) || ctx.input.ui_clicked("auto") {
            self.auto = !self.auto;
        }
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.keyboard = false;
        self.controller.update(&mut self.camera, &ctx.input, ctx.time.dt, over_ui);
        self.hover = if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)) };

        if let Phase::Over = self.phase {
            if ctx.input.key_pressed(Key::Enter)
                || ctx.input.ui_clicked("continue")
                || ctx.input.key_pressed(Key::Escape)
            {
                return Transition::PopWith(Box::new(self.result()));
            }
        } else if ctx.input.key_pressed(Key::Escape) || ctx.input.ui_clicked("retreat") {
            self.won = Some(false);
            return Transition::PopWith(Box::new(BattleResult { won: false, army: self.result().army }));
        }

        if let Phase::Input = self.phase {
            let cur_team = self.units[self.current].team;
            if cur_team == Team::Red || self.auto {
                let cmd = self.ai_command();
                self.execute(cmd);
            } else {
                self.preview = self.hover.and_then(|p| self.command_for(p));
                if ctx.input.key_pressed(Key::Space) || ctx.input.ui_clicked("wait") {
                    self.execute(Command::Wait);
                } else if ctx.input.clicked(MouseButton::Left)
                    && !over_ui
                    && let Some(c) = self.preview.take()
                {
                    self.execute(c);
                }
            }
        }
        common::music(ctx, self.art.sfx.battle_music);
        common::audio_keys(ctx);
        common::flush_sounds(ctx, &mut self.sounds, &self.camera);
        // Auto-battle plays at triple speed.
        self.step(ctx.time.dt * if self.auto { 3.0 } else { 1.0 });
        let side = |t: Team| self.units.iter().filter(|u| u.team == t && u.alive()).map(|u| u.stack.count).sum::<u32>();
        ctx.status = format!(
            "heroes-battle round={} current={} blue={} red={} over={}",
            self.round,
            self.units[self.current].stack.kind.name(),
            side(Team::Blue),
            side(Team::Red),
            self.won.map_or("no".into(), |w| if w { "won".into() } else { "lost".to_string() })
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        f.clear(Color::hex(0x1b2a1c));
        let occ = self.occupied();
        let input_phase =
            matches!(self.phase, Phase::Input) && self.units[self.current].team == Team::Blue && !self.auto;
        {
            let mut w = f.world(&mut self.world, &self.camera);
            // Grid.
            let hexes: Vec<Hex> = w.world.hexes().collect();
            for h in hexes {
                if w.world.tile(h).is_some_and(|t| t.height < 2) {
                    w.hex_outline(h, Color::BLACK.with_alpha(0.22), 0.04);
                }
            }
            if !matches!(self.phase, Phase::Over) {
                for h in self.reach.hexes() {
                    w.hex_fill_inset(h, Color::hex(0x9fd8ff).with_alpha(0.22), 0.08);
                }
                let cur = &self.units[self.current];
                w.hex_outline(cur.hex, Color::hex(0xffe27a), 0.14);
            }
            if input_phase {
                match &self.preview {
                    Some(Command::Move(p)) => {
                        w.path(p, 0.08, Color::WHITE.with_alpha(0.8), false);
                        w.hex_fill(*p.last().unwrap(), Color::WHITE.with_alpha(0.25));
                    }
                    Some(Command::MoveAttack(p, t)) => {
                        w.path(p, 0.08, Color::hex(0xffb070).with_alpha(0.8), false);
                        w.hex_fill(*p.last().unwrap(), Color::hex(0xffb070).with_alpha(0.3));
                        w.hex_outline(self.units[*t].hex, Color::RED, 0.14);
                        w.label(
                            w.world.hex_to_world(self.units[*t].hex) + Vec3::Y * 1.9,
                            "Attack",
                            8.0,
                            Color::hex(0xffb070),
                        );
                    }
                    Some(Command::Shoot(t)) => {
                        let (a, b) = (
                            self.units[self.current].actor.pos + Vec3::Y * 0.6,
                            self.units[*t].actor.pos + Vec3::Y * 0.6,
                        );
                        w.line(a, b, 0.05, Color::hex(0xffe27a).with_alpha(0.7));
                        w.hex_outline(self.units[*t].hex, Color::RED, 0.14);
                        w.label(self.units[*t].actor.pos + Vec3::Y * 1.9, "Shoot", 8.0, Color::hex(0xffe27a));
                    }
                    _ => {
                        if let Some(p) = self.hover {
                            w.hex_outline(p.hex, Color::WHITE.with_alpha(0.3), 0.06);
                        }
                    }
                }
            }
            // Dead first so the living stand on top of corpses.
            let mut order: Vec<usize> = (0..self.units.len()).collect();
            order.sort_by_key(|&i| self.units[i].alive());
            for i in order {
                let u = &self.units[i];
                u.actor.draw(&mut w, &self.art, false);
                if u.alive() {
                    let badge = u.actor.pos + Vec3::new(if u.team == Team::Blue { 0.45 } else { -0.45 }, 0.0, 0.45);
                    let col = if u.team == Team::Blue { Color::hex(0xbfd8ff) } else { Color::hex(0xffc0b0) };
                    w.label(badge, &format!("{}", u.stack.count), 8.0, col);
                }
            }
            // Arrow in flight.
            if let Phase::Striking { attacker, target, ranged: true, t, .. } = self.phase {
                let k = ((t - 0.08) / 0.17).clamp(0.0, 1.0);
                if k > 0.0 && k < 1.0 {
                    let (a, b) =
                        (self.units[attacker].actor.pos + Vec3::Y * 0.9, self.units[target].actor.pos + Vec3::Y * 0.9);
                    let p = a.lerp(b, k) + Vec3::Y * (k * (1.0 - k) * 2.0);
                    w.sprite(Sprite::new(self.art.arrow, p).anchor(0.5, 0.5).scale(0.7).flip(b.x < a.x));
                }
            }
            let _ = occ;
            for fl in &self.floaters {
                w.label(fl.pos, &fl.text, 16.0, fl.color.with_alpha((1.2 - fl.t).clamp(0.0, 1.0)));
            }
        }

        let size = f.ui_size();
        let t = f.theme.clone();
        // Top bar: round and whose turn.
        let bar = Rect::new(size.x * 0.5 - 230.0, 8.0, 460.0, 58.0);
        f.panel(bar);
        let cur = &self.units[self.current];
        let img = self.art.unit(cur.stack.kind, cur.team).idle.frame_at(0.0);
        f.image_fit(Rect::new(bar.x + 6.0, bar.y + 2.0, 54.0, 54.0), img, Color::WHITE);
        let who = if cur.team == Team::Blue { "Your" } else { "Enemy" };
        f.text(
            Vec2::new(bar.x + 66.0, bar.y + 10.0),
            &format!("Round {}  -  {} {} x{}", self.round, who, cur.stack.kind.name(), cur.stack.count),
            8.0,
            t.accent,
        );
        let s = stats(cur.stack.kind);
        f.text(
            Vec2::new(bar.x + 66.0, bar.y + 26.0),
            &format!(
                "hp {}  dmg {}-{}  speed {}{}",
                s.hp,
                s.min_dmg,
                s.max_dmg,
                s.speed,
                if s.ranged { "  ranged" } else { "" }
            ),
            8.0,
            t.text_dim,
        );
        let btns = Rect::new(bar.x + 66.0, bar.y + 36.0, 380.0, 18.0).cols(3, 6.0);
        f.button_ex("wait", btns[0], "Defend [Space]", input_phase, false);
        f.button_ex("auto", btns[1], "Auto [A]", true, self.auto);
        f.button_ex("retreat", btns[2], "Retreat [Esc]", !matches!(self.phase, Phase::Over), false);

        // Combat log.
        let log = Rect::new(8.0, size.y - 88.0, 440.0, 80.0);
        f.panel(log);
        for (i, l) in self.log.iter().enumerate() {
            let a = 1.0 - i as f32 * 0.17;
            f.text(Vec2::new(log.x + 10.0, log.y + 10.0 + i as f32 * 13.0), l, 8.0, t.text.with_alpha(a));
        }
        common::stats(f);

        if let Phase::Over = self.phase {
            let won = self.won == Some(true);
            let r = Rect::new(size.x * 0.5 - 180.0, size.y * 0.5 - 90.0, 360.0, 180.0);
            f.panel(r);
            f.text_centered(
                Rect::new(r.x, r.y + 16.0, r.w, 30.0),
                if won { "VICTORY" } else { "DEFEAT" },
                24.0,
                if won { t.accent } else { Color::RED },
            );
            let lost: u32 =
                self.units.iter().filter(|u| u.team == Team::Blue).map(|u| u.start_count - u.stack.count).sum();
            let slain: u32 =
                self.units.iter().filter(|u| u.team == Team::Red).map(|u| u.start_count - u.stack.count).sum();
            f.text_centered(
                Rect::new(r.x, r.y + 64.0, r.w, 20.0),
                &format!("Enemies slain: {slain}   Your losses: {lost}"),
                8.0,
                t.text,
            );
            f.button("continue", Rect::new(r.x + 90.0, r.y + 110.0, 180.0, 40.0), "Continue");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::UnitKind;

    /// Auto-battles must always finish (no stuck turns).
    #[test]
    fn auto_battles_finish() {
        let mut ctx = Context::new();
        let art = Art::load(&mut ctx);
        for (army, enemy) in [
            (vec![Stack::new(UnitKind::Footman, 14), Stack::new(UnitKind::Archer, 8)], Stack::new(UnitKind::Wolf, 6)),
            (vec![Stack::new(UnitKind::Footman, 5)], Stack::new(UnitKind::Ogre, 8)),
            (vec![Stack::new(UnitKind::Knight, 3), Stack::new(UnitKind::Archer, 10)], Stack::new(UnitKind::Griffon, 2)),
        ] {
            let mut b = BattleScene::new(art.clone(), army, vec![enemy]);
            b.auto = true;
            ctx.time.dt = 1.0 / 30.0;
            let mut frames = 0;
            while b.won.is_none() {
                let _ = b.update(&mut ctx);
                frames += 1;
                let phase = match b.phase {
                    Phase::Input => "input",
                    Phase::Moving { .. } => "moving",
                    Phase::Striking { .. } => "striking",
                    Phase::Pause(_) => "pause",
                    Phase::Over => "over",
                };
                assert!(
                    frames < 30 * 600,
                    "battle stuck: round {} unit {} phase {phase}",
                    b.round,
                    b.units[b.current].stack.kind.name()
                );
            }
        }
    }
}
