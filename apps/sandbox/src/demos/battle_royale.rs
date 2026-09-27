//! Battle royale, turn based: twelve heroes, free-for-all, fog of war. A ring of fire closes in
//! towards a random point ([`ShrinkingZone`] + [`Wildfire`]): the next safe circle is shown in
//! advance, standing outside it hurts every turn, and the fire spreads by terrain — forests
//! burn long, grass burns fast, rock and water don't. Loot chests make the early turns matter.
//! Last hero standing wins.

use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path::Reachable;
use knight_engine::hex::vision::field_of_view;
use knight_engine::hex::{Hex, HexMap, HexSet, Rng};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{self, Anim, Decor, MapGen};
use crate::tactics::{self, Action, Director, Event, Unit};

const YOU: usize = 0;
const PLAYERS: usize = 12;
const SIGHT: i32 = 7;
const NAMES: [&str; PLAYERS] =
    ["You", "Ash", "Brine", "Cinder", "Dusk", "Ember", "Flint", "Gale", "Haze", "Ivy", "Jinx", "Knox"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    You,
    Others,
    Hazard,
    Over,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Loot {
    Potion,
    Sword,
    Boots,
}

pub struct BattleRoyale {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    camera: Camera,
    minimap: Camera,
    controller: CameraController,
    units: Vec<Unit>,
    director: Director,
    zone: ShrinkingZone,
    fire: Wildfire,
    fx: Particles,
    loot: HexMap<Loot>,
    phase: Phase,
    turn: u32,
    reach: Reachable,
    preview: Vec<Action>,
    hover: Option<Pick>,
    /// Finishing place of eliminated players (1 = winner).
    place: Vec<Option<usize>>,
    log: Vec<String>,
    ash_mat: MaterialId,
    fire_t: f32,
    rng: Rng,
}

impl BattleRoyale {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let ash_mat = world.add_material(Material::color("ash", Color::hex(0x3a3431), Color::hex(0x2a2522)));
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 404, size: 18, amplitude: 7.0, island: 0.85, trees: 0.55, ..Default::default() },
        );
        world.set_fog_enabled(true);
        let land: HexSet =
            world.hexes().filter(|&h| world.tile(h).is_some_and(|t| t.height >= 1) && !world.is_submerged(h)).collect();
        let mut sorted: Vec<Hex> = land.iter().copied().collect();
        sorted.sort();
        let mut rng = Rng::new(77);

        // Spawn around the island, far from the middle and from each other.
        let mut spawns: Vec<Hex> = Vec::new();
        let mut ring: Vec<Hex> = sorted.iter().copied().filter(|h| (10..=15).contains(&h.length())).collect();
        rng.shuffle(&mut ring);
        for h in ring {
            if spawns.len() < PLAYERS && spawns.iter().all(|s| s.distance(h) >= 5) {
                spawns.push(h);
            }
        }
        while spawns.len() < PLAYERS {
            let h = sorted[rng.range(0, sorted.len() as i32) as usize];
            if !spawns.contains(&h) {
                spawns.push(h);
            }
        }
        let kinds = [
            UnitKind::Paladin,
            UnitKind::Knight,
            UnitKind::Archer,
            UnitKind::Ogre,
            UnitKind::Footman,
            UnitKind::Griffon,
        ];
        let units: Vec<Unit> = (0..PLAYERS)
            .map(|i| {
                let team = if i == YOU { Team::Blue } else { Team::Red };
                let mut u = Unit::new(
                    &world,
                    if i == YOU { UnitKind::Paladin } else { kinds[i % kinds.len()] },
                    team,
                    spawns[i],
                );
                u.faction = i as u32;
                u.actor.hp = 40.0;
                u.actor.max_hp = 40.0;
                u.moves = 4;
                u.actor.scale = 0.9;
                u
            })
            .collect();
        let mut loot = HexMap::new();
        for k in 0..22 {
            let h = sorted[rng.range(0, sorted.len() as i32) as usize];
            if h.length() <= 12 && !spawns.contains(&h) {
                loot.insert(h, [Loot::Potion, Loot::Sword, Loot::Boots][k % 3]);
            }
        }
        let mut decor = decor;
        decor.retain(|d| !spawns.contains(&d.hex) && !loot.contains(d.hex));
        let zone = ShrinkingZone::new(
            Hex::ORIGIN,
            17,
            vec![
                ZonePhase { wait: 3, shrink: 3, radius: 11, damage: 3.0 },
                ZonePhase { wait: 3, shrink: 3, radius: 7, damage: 5.0 },
                ZonePhase { wait: 2, shrink: 3, radius: 4, damage: 8.0 },
                ZonePhase { wait: 2, shrink: 2, radius: 2, damage: 12.0 },
                ZonePhase { wait: 2, shrink: 2, radius: 0, damage: 20.0 },
            ],
            2024,
            Some(land),
        );
        let camera = Camera::new(world.hex_to_world(spawns[YOU]), 46.0).with_pitch(52.0);
        let mut minimap = Camera::new(world.center(), 4.0).with_pitch(90.0);
        minimap.min_zoom = 0.5;
        let mut demo = BattleRoyale {
            art,
            world,
            decor,
            camera,
            minimap,
            controller: CameraController::default().drag_with(&[MouseButton::Right, MouseButton::Middle]),
            units,
            director: Director::new(8),
            zone,
            fire: Wildfire::new(5),
            fx: Particles::new(),
            loot,
            phase: Phase::You,
            turn: 1,
            reach: Reachable::default(),
            preview: Vec::new(),
            hover: None,
            place: vec![None; PLAYERS],
            log: vec!["The fire is coming. Find the safe zone!".into()],
            ash_mat,
            fire_t: 0.0,
            rng,
        };
        demo.director.speed = 2.0;
        demo.start_your_turn();
        demo.update_fog();
        demo
    }

    fn alive(&self) -> usize {
        self.units.iter().filter(|u| u.alive()).count()
    }

    fn say(&mut self, s: String) {
        self.log.push(s);
        if self.log.len() > 6 {
            self.log.remove(0);
        }
    }

    /// Where standing is safe: inside the zone and not on fire.
    fn safe(&self, h: Hex) -> bool {
        self.zone.contains(h) && !self.fire.is_burning(h)
    }

    fn flammability(world: &HexWorld, h: Hex) -> f32 {
        if world.is_submerged(h) {
            return 0.0;
        }
        match world.tile(h).map(|t| world.materials[t.material as usize].name.as_str()) {
            Some("forest") => 1.0,
            Some("meadow") => 0.85,
            Some("grass") => 0.7,
            Some("dirt") => 0.35,
            Some("sand" | "road" | "rock") => 0.12,
            _ => 0.0,
        }
    }

    fn update_fog(&mut self) {
        let me = &self.units[YOU];
        if !me.alive() {
            // Spectating: see everything.
            self.world.set_fog_enabled(false);
            return;
        }
        let at = me.hex;
        let world = &self.world;
        let origin = world.height(at).unwrap_or(0) as f32;
        let visible = field_of_view(at, SIGHT, 1.5, |h| {
            if h == at { origin } else { world.height(h).map_or(-100.0, |x| x.max(0) as f32) }
        });
        let visible: Vec<Hex> = visible.into_iter().filter(|h| self.world.contains(*h)).collect();
        self.world.update_fog(visible);
    }

    fn start_your_turn(&mut self) {
        self.phase = Phase::You;
        for u in &mut self.units {
            u.moved = false;
            u.acted = false;
        }
        if self.units[YOU].alive() {
            self.reach = tactics::reach(&self.world, &self.units, YOU, self.units[YOU].moves);
        } else {
            self.reach = Reachable::default();
            self.end_your_turn();
        }
    }

    fn end_your_turn(&mut self) {
        self.phase = Phase::Others;
        self.preview.clear();
        let refuge = Some(self.zone.next().0);
        let safe = |h: Hex| self.zone.contains(h) && !self.fire.is_burning(h);
        let mut plans = Vec::new();
        let mut board = self.units.clone();
        for i in 1..PLAYERS {
            if board[i].alive() {
                for a in tactics::ai_turn_with(&self.world, &board, i, &safe, refuge, Some(SIGHT)) {
                    if let Action::Move { unit, path } = &a {
                        board[*unit].hex = *path.last().unwrap();
                    }
                    plans.push(a);
                }
            }
        }
        for a in plans {
            self.director.push(a);
        }
    }

    fn pick_up(&mut self, sounds: &mut Vec<(SoundId, Vec3)>) {
        for i in 0..PLAYERS {
            let u = &self.units[i];
            if !u.alive() {
                continue;
            }
            if let Some(item) = self.loot.remove(u.hex) {
                let pos = u.actor.pos;
                let u = &mut self.units[i];
                let text = match item {
                    Loot::Potion => {
                        u.actor.hp = (u.actor.hp + 20.0).min(u.actor.max_hp);
                        "+20 hp"
                    }
                    Loot::Sword => {
                        u.damage = (u.damage.0 + 3, u.damage.1 + 3);
                        "+3 damage"
                    }
                    Loot::Boots => {
                        u.moves += 1;
                        "+1 move"
                    }
                };
                self.director.float(pos, text.to_string(), Color::hex(0x9fffa0));
                sounds.push((self.art.sfx.powerup, pos));
            }
        }
    }

    /// End of round: the zone advances, the fire spreads, everyone outside gets burned.
    fn hazard(&mut self, sounds: &mut Vec<(SoundId, Vec3)>) {
        let dmg = self.zone.damage();
        for i in 0..PLAYERS {
            let (h, alive) = (self.units[i].hex, self.units[i].alive());
            if alive && !self.safe(h) {
                let u = &mut self.units[i];
                u.actor.hp -= dmg;
                let pos = u.actor.pos;
                u.actor.play(if u.alive() { Anim::Hurt } else { Anim::Death });
                self.director.float(pos, format!("-{dmg:.0} burn"), Color::hex(0xffa040));
                sounds.push((self.art.sfx.hit, pos));
            }
        }
        match self.zone.tick() {
            ZoneEvent::ShrinkStarted { .. } => {
                self.say("The fire closes in!".into());
                sounds.push((self.art.sfx.explosion, self.world.hex_to_world(self.units[YOU].hex)));
            }
            ZoneEvent::ShrinkEnded { .. } if !self.zone.is_final() => {
                let t = self.zone.ticks_until_shrink();
                self.say(format!("The zone holds for {t} turns."));
            }
            _ => {}
        }
        // Ignite the land outside the zone: a ragged fire front, then spread by terrain.
        let world = &self.world;
        let outside: Vec<Hex> = world
            .hexes()
            .filter(|&h| !self.zone.contains(h) && !self.fire.is_ash(h) && !self.fire.is_burning(h))
            .collect();
        for h in outside {
            let depth = h.distance(self.zone.center) - self.zone.radius;
            let f = Self::flammability(world, h);
            if f > 0.0 && (depth >= 2 || self.rng.chance(0.5)) {
                self.fire.ignite(h, f);
            }
        }
        let zone = &self.zone;
        self.fire.tick(|h| Self::flammability(world, h), |h| !zone.contains(h));
        // Burnt-out land turns to ash (and trees on it are gone).
        let ash: Vec<Hex> = self
            .fire
            .ash()
            .copied()
            .filter(|h| self.world.tile(*h).is_some_and(|t| t.material != self.ash_mat))
            .collect();
        for h in ash {
            self.world.set_material(h, self.ash_mat);
        }
        let fire = &self.fire;
        self.decor.retain(|d| !fire.is_ash(d.hex));
    }

    fn record_deaths(&mut self) {
        let left = self.alive();
        for (i, name) in NAMES.iter().enumerate() {
            if !self.units[i].alive() && self.place[i].is_none() {
                self.place[i] = Some(left + 1);
                let msg = if i == YOU {
                    format!("You were eliminated - #{}", left + 1)
                } else {
                    format!("{name} is out (#{})", left + 1)
                };
                self.say(msg);
            }
        }
        if left <= 1 && self.phase != Phase::Over {
            if let Some(w) = (0..PLAYERS).find(|&i| self.units[i].alive()) {
                self.place[w] = Some(1);
                self.say(format!("{} wins the battle royale!", NAMES[w]));
            }
            self.phase = Phase::Over;
        }
    }
}

impl Scene for BattleRoyale {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let dt = ctx.time.dt;
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        self.hover = if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)) };
        if ctx.input.key_pressed(Key::R) && self.phase == Phase::Over {
            *self = BattleRoyale::new(self.art.clone());
            return Transition::None;
        }

        let mut sounds = Vec::new();
        for e in self.director.update(&self.world, &self.art, &mut self.units, dt) {
            if let Event::Struck { pos, ranged } = e {
                common::strike_sound(ctx, &self.art, pos, ranged, &self.camera);
            }
        }
        self.record_deaths();
        let busy = self.director.busy();
        match self.phase {
            Phase::You if !busy => {
                if self.units[YOU].moved {
                    self.reach = Reachable::default();
                }
                self.preview = self
                    .hover
                    .map_or(Vec::new(), |p| tactics::player_command(&self.world, &self.units, YOU, &self.reach, p));
                if ctx.input.clicked(MouseButton::Left) && !over_ui && !self.preview.is_empty() {
                    for a in std::mem::take(&mut self.preview) {
                        self.units[YOU].moved = true;
                        if matches!(a, Action::Attack { .. }) {
                            self.units[YOU].acted = true;
                        }
                        self.director.push(a);
                    }
                } else if ctx.input.key_pressed(Key::Space) || ctx.input.ui_clicked("endturn") {
                    self.units[YOU].moved = true;
                    self.units[YOU].acted = true;
                }
                let me = &self.units[YOU];
                let can_hit = !me.acted
                    && (1..PLAYERS).any(|j| {
                        self.units[j].alive()
                            && self.world.is_visible(self.units[j].hex)
                            && tactics::in_range(&self.units, YOU, me.hex, j)
                    });
                if me.moved && !can_hit && !self.director.busy() {
                    self.pick_up(&mut sounds);
                    self.update_fog();
                    self.end_your_turn();
                }
            }
            Phase::Others if !busy => {
                self.pick_up(&mut sounds);
                self.phase = Phase::Hazard;
            }
            Phase::Hazard if !busy => {
                self.hazard(&mut sounds);
                self.record_deaths();
                if self.phase != Phase::Over {
                    self.turn += 1;
                    self.update_fog();
                    self.start_your_turn();
                }
            }
            _ => {}
        }
        common::flush_sounds(ctx, &mut sounds, &self.camera);

        // Flames on burning hexes near the camera.
        self.fire_t -= dt;
        if self.fire_t <= 0.0 {
            self.fire_t = 0.1;
            let view = self.camera.viewport.inset(-60.0);
            for h in self.fire.burning() {
                let p = self.world.hex_to_world(h);
                if view.contains(self.camera.world_to_screen(p)) {
                    let jitter = Vec3::new(self.rng.range_f32(-0.4, 0.4), 0.05, self.rng.range_f32(-0.3, 0.3));
                    self.fx.burst(p + jitter, &Burst { count: 2, ..Burst::fire() });
                    if self.rng.chance(0.08) {
                        self.fx.burst(p + Vec3::Y * 0.8, &Burst { count: 1, ..Burst::smoke() });
                    }
                }
            }
        }
        self.fx.update(dt);
        let focus = if self.units[YOU].alive() {
            YOU
        } else {
            (0..PLAYERS)
                .filter(|&i| self.units[i].alive())
                .max_by(|&a, &b| self.units[a].actor.hp.total_cmp(&self.units[b].actor.hp))
                .unwrap_or(YOU)
        };
        if self.units[focus].actor.moving() || !self.units[YOU].alive() {
            self.camera.move_to(self.units[focus].actor.pos);
        }
        common::music(ctx, self.art.sfx.battle_music);
        ctx.status = format!(
            "royale turn={} phase={:?} alive={} you_hp={:.0} zone=({},{})r{} next=({},{})r{} burning={} ash={} place={:?}",
            self.turn,
            self.phase,
            self.alive(),
            self.units[YOU].actor.hp.max(0.0),
            self.zone.center.q,
            self.zone.center.r,
            self.zone.radius,
            self.zone.next().0.q,
            self.zone.next().0.r,
            self.zone.next().1,
            self.fire.burning().count(),
            self.fire.ash().count(),
            self.place[YOU]
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let your_turn = self.phase == Phase::You && !self.director.busy() && self.units[YOU].alive();
        let (zc, zr) = (self.zone.center, self.zone.radius);
        let (nc, nr) = self.zone.next();
        let in_circle = |c: Hex, r: i32, w: &HexWorld| -> HexSet { c.range(r).filter(|h| w.contains(*h)).collect() };
        let zone_set = in_circle(zc, zr, &self.world);
        let next_set = in_circle(nc, nr, &self.world);
        let visible: Vec<bool> =
            self.units.iter().map(|u| u.faction == YOU as u32 || self.world.is_visible(u.hex)).collect();
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
            // Fire tint on burning hexes, ash stays dark through its material.
            let burning: Vec<Hex> = self.fire.burning().collect();
            for h in &burning {
                w.hex_fill(*h, Color::hex(0xff6a20).with_alpha(0.35));
            }
            for (h, item) in self.loot.iter() {
                if w.world.visibility(h) == Visibility::Hidden {
                    continue;
                }
                let img = match item {
                    Loot::Potion => self.art.emotes[0].1,
                    Loot::Sword => self.art.treasure,
                    Loot::Boots => self.art.emotes[7].1,
                };
                w.sprite(Sprite::new(img, w.world.hex_to_world(h)).scale(0.7));
            }
            // Current zone edge (white) and where it goes next (blue).
            w.set_on_top(true);
            w.region_outline(&zone_set, Color::WHITE.with_alpha(0.85), 0.08);
            if (nc, nr) != (zc, zr) {
                w.region_outline(&next_set, Color::hex(0x6fb0ff).with_alpha(0.9), 0.06);
            }
            if your_turn {
                for h in self.reach.hexes() {
                    let col = if self.zone.contains(h) && !self.fire.is_burning(h) {
                        Color::hex(0x9fd8ff)
                    } else {
                        Color::hex(0xff8040)
                    };
                    w.hex_fill_inset(h, col.with_alpha(0.18), 0.1);
                }
                for a in &self.preview {
                    match a {
                        Action::Move { path, .. } => {
                            w.path(path, 0.08, Color::WHITE.with_alpha(0.85), false);
                            w.hex_outline(*path.last().unwrap(), Color::WHITE, 0.1);
                        }
                        Action::Attack { target, .. } => w.hex_outline(self.units[*target].hex, Color::RED, 0.14),
                        Action::Nova { .. } => {}
                    }
                }
            }
            w.set_on_top(false);
            for (i, u) in self.units.iter().enumerate() {
                if !visible[i] {
                    continue;
                }
                let mut a = u.actor.clone();
                a.selected = i == YOU;
                a.draw(&mut w, &self.art, true);
                if u.alive() {
                    w.label(
                        u.actor.pos + Vec3::Z * 0.55,
                        NAMES[i],
                        8.0,
                        if i == YOU { Color::hex(0xffe27a) } else { Color::hex(0xffb0a0) },
                    );
                }
            }
            self.director.draw(&mut w, &self.art, &self.units);
            self.fx.draw(&mut w);
        }
        // Minimap: zones, fire and players you can see.
        let size = f.ui_size();
        let s = f.ui_scale();
        let mm = Rect::new(size.x - 208.0, size.y - 208.0, 200.0, 200.0);
        self.minimap.viewport = mm.scale(s);
        let (lo, hi) = self.world.bounds();
        self.minimap.target = Vec3::new((lo.x + hi.x) * 0.5, 0.0, (lo.y + hi.y) * 0.5);
        self.minimap.zoom = (mm.w * s / (hi.x - lo.x + 2.0)).min(mm.h * s / (hi.y - lo.y + 2.0));
        {
            let mut m = f.world(&mut self.world, &self.minimap);
            m.background(Color::hex(0x0a0a10));
            m.set_on_top(true);
            for h in self.fire.burning() {
                m.hex_fill(h, Color::hex(0xff6a20).with_alpha(0.8));
            }
            m.region_outline(&zone_set, Color::WHITE, 0.3);
            m.region_outline(&next_set, Color::hex(0x6fb0ff), 0.3);
            for (i, u) in self.units.iter().enumerate() {
                if u.alive() && visible[i] {
                    m.ring(u.actor.pos, 1.0, if i == YOU { Color::hex(0xffe27a) } else { Color::hex(0xff5a4a) });
                }
            }
        }
        f.block(mm);
        f.outline(mm.inset(-2.0), f.theme.panel_border, 2.0);

        common::header(f, "Battle Royale", "Free-for-all on hexes; a ring of fire closes in on a random point");
        common::stats(f);
        let t = f.theme.clone();
        // Top banner: players left and what the fire does next.
        let bar = Rect::new(size.x * 0.5 - 250.0, 70.0, 500.0, 46.0);
        f.panel(bar);
        let zone_text = if self.zone.is_final() {
            "Final circle".to_string()
        } else if self.zone.is_shrinking() {
            "The fire is closing in!".to_string()
        } else {
            format!("Fire moves in {} turn(s)", self.zone.ticks_until_shrink())
        };
        f.text(Vec2::new(bar.x + 12.0, bar.y + 8.0), &format!("{} LEFT", self.alive()), 16.0, t.accent);
        f.text(Vec2::new(bar.x + 150.0, bar.y + 8.0), &format!("Turn {}   {}", self.turn, zone_text), 8.0, t.text);
        f.text(
            Vec2::new(bar.x + 150.0, bar.y + 24.0),
            &format!("Burn outside the zone: {:.0} per turn", self.zone.damage()),
            8.0,
            Color::hex(0xffa060),
        );

        // You.
        let me = &self.units[YOU];
        let r = Rect::new(size.x - 624.0, size.y - 76.0, 400.0, 68.0);
        f.panel(r);
        f.bar(
            Rect::new(r.x + 12.0, r.y + 12.0, 180.0, 12.0),
            me.actor.hp / me.actor.max_hp,
            Color::hex(0xd8302a),
            Color::hex(0x201010),
        );
        f.text(
            Vec2::new(r.x + 12.0, r.y + 30.0),
            &format!("{:.0} hp  dmg {}-{}  moves {}", me.actor.hp.max(0.0), me.damage.0, me.damage.1, me.moves),
            8.0,
            t.text,
        );
        let status = match self.phase {
            Phase::Over => match self.place[YOU] {
                Some(1) => "VICTORY ROYALE!  R: play again".to_string(),
                Some(p) => format!("Finished #{p}  R: play again"),
                None => "Game over".into(),
            },
            _ if !me.alive() => format!("Eliminated #{} - spectating", self.place[YOU].unwrap_or(0)),
            Phase::You if your_turn => {
                if self.safe(me.hex) {
                    "Your turn".into()
                } else {
                    "Your turn - GET OUT OF THE FIRE!".into()
                }
            }
            _ => "Others move...".into(),
        };
        f.text(Vec2::new(r.x + 12.0, r.y + 46.0), &status, 8.0, t.accent);
        f.button_ex("endturn", Rect::new(r.x + 260.0, r.y + 12.0, 128.0, 44.0), "End turn", your_turn, false);

        let lg = Rect::new(8.0, 106.0, 300.0, 92.0);
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
                "Click a blue hex: move   Click an enemy: attack   Space: end turn",
                "White ring: safe zone now   Blue ring: next zone   Orange: fire",
                "Chests: potion (heart), sword, boots   Right drag: pan",
            ],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A whole match plays out: the zone shrinks, fire spreads, players die, one wins.
    #[test]
    fn a_match_finishes_with_one_winner() {
        let mut ctx = Context::new();
        let art = Art::load(&mut ctx);
        let mut br = BattleRoyale::new(art);
        ctx.time.dt = 1.0 / 20.0;
        for _ in 0..40_000 {
            if br.phase == Phase::You && !br.director.busy() {
                ctx.input.on_key(Key::Space, true);
            }
            let _ = br.update(&mut ctx);
            ctx.input.end_frame();
            ctx.input.on_key(Key::Space, false);
            if br.phase == Phase::Over {
                break;
            }
        }
        assert_eq!(br.phase, Phase::Over, "turn {} alive {}", br.turn, br.alive());
        assert!(br.alive() <= 1);
        assert!(br.fire.ash().count() > 50, "the fire burned the map");
        eprintln!("match over on turn {} with zone radius {} (phase {})", br.turn, br.zone.radius, br.zone.phase());
        assert!(br.zone.radius < 17, "the zone shrank");
        let mut seen: Vec<usize> = br.place.iter().flatten().copied().collect();
        seen.sort();
        seen.dedup();
        assert!(seen.len() >= PLAYERS - 1, "everyone got a placement: {:?}", br.place);
    }
}
