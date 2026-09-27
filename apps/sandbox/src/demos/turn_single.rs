//! Turn based, single unit: one hero on the hex grid. The hero moves (terrain costs movement
//! points) and attacks or casts, then every monster that noticed him takes its turn. Monsters keep
//! coming; level up by killing them.

use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path::Reachable;
use knight_engine::hex::{Hex, Rng};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{self, Decor, MapGen};
use crate::tactics::{self, Action, Director, Event, Unit};

const HERO: usize = 0;
const MAX_MANA: f32 = 40.0;
const NOVA_COST: f32 = 20.0;
const AGGRO: i32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Hero,
    Monsters,
}

pub struct TurnSingle {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    camera: Camera,
    controller: CameraController,
    units: Vec<Unit>,
    director: Director,
    phase: Phase,
    reach: Reachable,
    preview: Vec<Action>,
    hover: Option<Pick>,
    turn: u32,
    mana: f32,
    level: u32,
    xp: u32,
    kills: u32,
    rng: Rng,
    show_threat: bool,
}

impl TurnSingle {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 5, size: 22, amplitude: 8.0, island: 0.8, trees: 0.45, ..Default::default() },
        );
        let start = world
            .hexes()
            .filter(|&h| world.tile(h).is_some_and(|t| t.height >= 1))
            .min_by_key(|h| (h.length(), h.q, h.r))
            .unwrap_or(Hex::ORIGIN);
        let hero = Unit::new(&world, UnitKind::Paladin, Team::Blue, start);
        let camera = Camera::new(hero.actor.pos, 56.0).with_pitch(50.0);
        let mut demo = TurnSingle {
            art,
            world,
            decor,
            camera,
            controller: CameraController::default().drag_with(&[MouseButton::Right, MouseButton::Middle]),
            units: vec![hero],
            director: Director::new(3),
            phase: Phase::Hero,
            reach: Reachable::default(),
            preview: Vec::new(),
            hover: None,
            turn: 1,
            mana: MAX_MANA,
            level: 1,
            xp: 0,
            kills: 0,
            rng: Rng::new(77),
            show_threat: true,
        };
        for _ in 0..5 {
            demo.spawn();
        }
        demo.start_hero_turn();
        demo
    }

    fn spawn(&mut self) {
        let occ = tactics::occupancy(&self.units);
        let center = self.units[HERO].hex;
        for _ in 0..40 {
            let h = center + Hex::new(self.rng.range(-12, 13), self.rng.range(-12, 13));
            let d = h.distance(center);
            if !(6..=12).contains(&d) || occ.contains(h) {
                continue;
            }
            if self.world.tile(h).is_some_and(|t| t.height >= 1) && !self.world.is_submerged(h) {
                let brute = self.rng.chance(0.25);
                let kind = if brute { UnitKind::Ogre } else { UnitKind::Wolf };
                let mut u = Unit::new(&self.world, kind, Team::Red, h);
                let bonus = (self.level - 1) as f32 * 3.0;
                u.actor.hp += bonus;
                u.actor.max_hp += bonus;
                u.actor.scale = if brute { 1.0 } else { 0.85 };
                self.units.push(u);
                return;
            }
        }
    }

    fn start_hero_turn(&mut self) {
        self.phase = Phase::Hero;
        let h = &mut self.units[HERO];
        h.moved = false;
        h.acted = false;
        self.reach = tactics::reach(&self.world, &self.units, HERO, self.units[HERO].moves);
    }

    fn end_hero_turn(&mut self) {
        self.phase = Phase::Monsters;
        self.preview.clear();
        // Every awake monster plans in turn, seeing where earlier monsters will stand; the
        // director then plays all the plans in order.
        let hero_hex = self.units[HERO].hex;
        for i in 1..self.units.len() {
            if self.units[i].alive() && self.units[i].hex.distance(hero_hex) <= AGGRO {
                for a in tactics::ai_turn(&self.world, &self.units, i) {
                    if let Action::Move { unit, path } = &a {
                        self.units[*unit].hex = *path.last().unwrap();
                    }
                    self.director.push(a);
                }
            }
        }
        // Restore current positions; the director moves units as the plans play out.
        for u in &mut self.units {
            u.hex = u.actor.hex();
        }
    }

    fn handle(&mut self, events: Vec<Event>) {
        for e in events {
            if let Event::Killed { unit, by } = e
                && by == HERO
                && unit != HERO
            {
                self.kills += 1;
                self.xp += self.units[unit].xp;
                let need = self.level * 40;
                if self.xp >= need {
                    self.xp -= need;
                    self.level += 1;
                    let h = &mut self.units[HERO];
                    h.actor.max_hp += 10.0;
                    h.actor.hp = h.actor.max_hp;
                    h.damage = (h.damage.0 + 2, h.damage.1 + 3);
                    let p = h.actor.pos;
                    self.director.float(p + Vec3::Y * 0.5, format!("LEVEL {}!", self.level), Color::YELLOW);
                }
            }
        }
    }
}

impl Scene for TurnSingle {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let dt = ctx.time.dt;
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        if self.units[HERO].actor.moving() {
            self.camera.move_to(self.units[HERO].actor.pos);
        }
        self.hover = if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)) };
        if ctx.input.key_pressed(Key::H) || ctx.input.ui_clicked("threat") {
            self.show_threat = !self.show_threat;
        }

        let events = self.director.update(&self.world, &self.art, &mut self.units, dt);
        for e in &events {
            if let Event::Struck { pos, ranged } = e {
                common::strike_sound(ctx, &self.art, *pos, *ranged, &self.camera);
            }
        }
        self.handle(events);
        if !self.units[HERO].alive() {
            if ctx.input.ui_clicked("respawn") || ctx.input.key_pressed(Key::Enter) {
                *self = TurnSingle::new(self.art.clone());
            }
        } else if self.phase == Phase::Hero && !self.director.busy() {
            if self.units[HERO].moved {
                self.reach = Reachable::default();
            }
            self.preview = self
                .hover
                .map_or(Vec::new(), |p| tactics::player_command(&self.world, &self.units, HERO, &self.reach, p));
            let input = &ctx.input;
            let cast = input.key_pressed(Key::Q) || input.ui_clicked("nova");
            if cast && self.mana >= NOVA_COST && !self.units[HERO].acted {
                self.mana -= NOVA_COST;
                let dmg = self.units[HERO].damage;
                ctx.audio.play(self.art.sfx.magic);
                self.director.push(Action::Nova { unit: HERO, radius: 2, damage: (dmg.0 - 2, dmg.1 - 2) });
                self.units[HERO].acted = true;
                self.units[HERO].moved = true;
            } else if input.clicked(MouseButton::Left) && !over_ui && !self.preview.is_empty() {
                for a in std::mem::take(&mut self.preview) {
                    self.units[HERO].moved = true;
                    if matches!(a, Action::Attack { .. }) {
                        self.units[HERO].acted = true;
                    }
                    self.director.push(a);
                }
            } else if input.key_pressed(Key::Space) || input.ui_clicked("endturn") {
                self.units[HERO].moved = true;
                self.units[HERO].acted = true;
            }
            // Once the hero has moved and has nothing left to hit, the monsters take their turn.
            let h = &self.units[HERO];
            let can_attack = !h.acted
                && (1..self.units.len())
                    .any(|j| self.units[j].alive() && tactics::in_range(&self.units, HERO, h.hex, j));
            if h.moved && !can_attack && !self.director.busy() {
                self.end_hero_turn();
            }
        } else if self.phase == Phase::Monsters && !self.director.busy() {
            self.turn += 1;
            self.mana = (self.mana + 5.0).min(MAX_MANA);
            let h = &mut self.units[HERO].actor;
            h.hp = (h.hp + 2.0).min(h.max_hp);
            // Drop corpses (indices shift, so only between turns when nothing is queued).
            self.units.retain(|u| u.alive() || u.actor.anim_t < 2.0);
            let living = self.units.iter().filter(|u| u.team() == Team::Red && u.alive()).count();
            if self.turn.is_multiple_of(2) && living < 10 {
                self.spawn();
            }
            self.start_hero_turn();
        }
        ctx.status = format!(
            "rpg turn={} phase={:?} hp={:.0} mana={:.0} level={} kills={} monsters={}",
            self.turn,
            self.phase,
            self.units[HERO].actor.hp.max(0.0),
            self.mana,
            self.level,
            self.kills,
            self.units.iter().filter(|u| u.team() == Team::Red && u.alive()).count()
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let hero_turn = self.phase == Phase::Hero && !self.director.busy() && self.units[HERO].alive();
        let threat = match (self.show_threat && hero_turn, self.hover) {
            (true, Some(p)) => tactics::occupancy(&self.units)
                .get(p.hex)
                .copied()
                .filter(|&m| m != HERO)
                .map(|m| tactics::reach(&self.world, &self.units, m, self.units[m].moves)),
            _ => None,
        };
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
            if hero_turn {
                for h in self.reach.hexes() {
                    w.hex_fill_inset(h, Color::hex(0x9fd8ff).with_alpha(0.2), 0.08);
                }
                if let Some(r) = &threat {
                    for h in r.hexes() {
                        w.hex_fill_inset(h, Color::hex(0xff5a4a).with_alpha(0.2), 0.12);
                    }
                }
                w.set_on_top(true);
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
                w.set_on_top(false);
            }
            for u in &self.units {
                u.actor.draw(&mut w, &self.art, u.team() == Team::Red);
            }
            self.director.draw(&mut w, &self.art, &self.units);
        }
        common::header(
            f,
            "Turn-Based - Single Unit",
            "You act, then every monster that noticed you acts; terrain costs movement",
        );
        common::stats(f);
        common::help(
            f,
            &[
                "Click a blue hex: move   Click a monster: (walk and) attack",
                "Q: Frost Nova (hits all within 2)   Space: end turn",
                "Hover a monster: its move range (H toggles)   Right drag: pan",
            ],
        );

        let size = f.ui_size();
        let t = f.theme.clone();
        let bar = Rect::new(size.x * 0.5 - 250.0, size.y - 96.0, 500.0, 88.0);
        f.panel(bar);
        let globe = |f: &mut Frame, r: Rect, frac: f32, color: Color, text: String| {
            let circle = f.assets().builtin.circle;
            f.image(r, circle, Color::hex(0x100c14));
            let frac = frac.clamp(0.0, 1.0);
            let sub = Rect::new(0.0, 1.0 - frac, 1.0, frac);
            f.image_sub(Rect::new(r.x, r.y + r.h * (1.0 - frac), r.w, r.h * frac), circle, sub, color);
            f.text_centered(r, &text, 8.0, Color::WHITE);
        };
        let hero = &self.units[HERO];
        let hp = hero.actor.hp / hero.actor.max_hp;
        globe(
            f,
            Rect::new(bar.x + 8.0, bar.y - 24.0, 104.0, 104.0),
            hp,
            Color::hex(0xd8302a),
            format!("{:.0}", hero.actor.hp.max(0.0)),
        );
        let mana = self.mana / MAX_MANA;
        globe(
            f,
            Rect::new(bar.x + bar.w - 112.0, bar.y - 24.0, 104.0, 104.0),
            mana,
            Color::hex(0x3a6ae8),
            format!("{:.0}", self.mana),
        );
        let slot = Rect::new(bar.x + 124.0, bar.y + 10.0, 52.0, 52.0);
        let can_cast = hero_turn && self.mana >= NOVA_COST && !hero.acted;
        f.button_ex("nova", slot, "", can_cast, false);
        f.image_fit(slot.inset(8.0), self.art.bolt, if can_cast { Color::WHITE } else { Color::rgb(0.3, 0.3, 0.3) });
        f.text(Vec2::new(slot.x + 4.0, slot.y + 40.0), "Q", 8.0, t.text);
        f.button_ex("endturn", Rect::new(bar.x + 124.0, bar.y + 66.0, 52.0, 18.0), "End", hero_turn, false);
        let status = if !hero.alive() {
            "Fallen".to_string()
        } else if hero_turn {
            format!("Your turn {}", self.turn)
        } else {
            "Monsters act...".to_string()
        };
        f.text(Vec2::new(bar.x + 190.0, bar.y + 12.0), &status, 16.0, if hero_turn { t.accent } else { t.text_dim });
        f.text(
            Vec2::new(bar.x + 190.0, bar.y + 36.0),
            &format!("Level {}   Kills {}", self.level, self.kills),
            8.0,
            t.text,
        );
        let xp = self.xp as f32 / (self.level * 40) as f32;
        f.bar(Rect::new(bar.x + 190.0, bar.y + 52.0, 180.0, 8.0), xp, Color::hex(0xf0c65a), Color::hex(0x202020));
        let steps = if hero.moved { "moved".to_string() } else { format!("{} moves", hero.moves) };
        let act = if hero.acted { "attacked" } else { "can attack" };
        f.text(Vec2::new(bar.x + 190.0, bar.y + 66.0), &format!("{steps}, {act}"), 8.0, t.text_dim);
        f.button_ex(
            "threat",
            Rect::new(size.x - 208.0, 106.0, 200.0, 24.0),
            "Show monster range [H]",
            true,
            self.show_threat,
        );

        if !hero.alive() {
            let r = Rect::new(size.x * 0.5 - 160.0, size.y * 0.5 - 70.0, 320.0, 140.0);
            f.panel(r);
            f.text_centered(Rect::new(r.x, r.y + 20.0, r.w, 30.0), "YOU DIED", 24.0, Color::RED);
            f.button("respawn", Rect::new(r.x + 80.0, r.y + 80.0, 160.0, 36.0), "Try again");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Skipping turns lets monsters come and attack; the turn loop never stalls.
    #[test]
    fn monsters_take_turns() {
        let mut ctx = Context::new();
        let art = Art::load(&mut ctx);
        let mut d = TurnSingle::new(art);
        ctx.time.dt = 1.0 / 20.0;
        let hp0 = d.units[HERO].actor.hp;
        for _ in 0..3000 {
            if d.phase == Phase::Hero && !d.director.busy() {
                ctx.input.on_key(Key::Space, true);
            }
            let _ = d.update(&mut ctx);
            ctx.input.end_frame();
            ctx.input.on_key(Key::Space, false);
            if d.turn > 12 || !d.units[HERO].alive() {
                break;
            }
        }
        assert!(d.turn > 3, "turns advanced: {}", d.turn);
        assert!(d.units[HERO].actor.hp < hp0 || !d.units[HERO].alive(), "monsters attacked");
    }
}
