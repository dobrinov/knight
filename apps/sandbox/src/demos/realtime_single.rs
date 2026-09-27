//! Real time, single unit: control one hero on the hex grid in real time. Click a hex to walk
//! there (hex by hex, slower in forests and snow, faster on roads), click a monster to chase and
//! strike it. Monsters roam, notice you and hunt you. Space pauses; orders can be given paused.

use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::{Hex, Rng};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{self, Decor, MapGen};
use crate::realtime::{self, Order, RtEvent, RtWorld};

const HERO: usize = 0;
const NOVA_COOLDOWN: f32 = 6.0;

pub struct RealtimeSingle {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    camera: Camera,
    controller: CameraController,
    rt: RtWorld,
    fx: Particles,
    hover: Option<Pick>,
    nova: f32,
    kills: u32,
    spawn_timer: f32,
    rng: Rng,
    resume_speed: f32,
}

impl RealtimeSingle {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 5, size: 22, amplitude: 8.0, island: 0.8, trees: 0.45, ..Default::default() },
        );
        // A road so the speed difference is easy to feel.
        for q in -14..=14 {
            let h = Hex::new(q, -q / 3);
            if world.tile(h).is_some_and(|t| t.height >= 1) {
                world.set_material(h, mats.road);
            }
        }
        let start = world
            .hexes()
            .filter(|&h| world.tile(h).is_some_and(|t| t.height >= 1))
            .min_by_key(|h| (h.length(), h.q, h.r))
            .unwrap_or(Hex::ORIGIN);
        let mut rt = RtWorld::new();
        rt.spawn(&world, UnitKind::Paladin, Team::Blue, start);
        rt.units[HERO].aggro = 0;
        let camera = Camera::new(world.hex_to_world(start), 52.0).with_pitch(50.0);
        let mut demo = RealtimeSingle {
            art,
            world,
            decor,
            camera,
            controller: CameraController::default().drag_with(&[MouseButton::Right, MouseButton::Middle]),
            rt,
            fx: Particles::new(),
            hover: None,
            nova: 0.0,
            kills: 0,
            spawn_timer: 0.0,
            rng: Rng::new(4),
            resume_speed: 1.0,
        };
        for _ in 0..6 {
            demo.spawn();
        }
        demo
    }

    fn spawn(&mut self) {
        let center = self.rt.units[HERO].hex();
        for _ in 0..40 {
            let h = center + Hex::new(self.rng.range(-12, 13), self.rng.range(-12, 13));
            if !(7..=12).contains(&h.distance(center)) || self.world.tile(h).is_none_or(|t| t.height < 1) {
                continue;
            }
            let kind = if self.rng.chance(0.25) { UnitKind::Ogre } else { UnitKind::Wolf };
            if self.rt.spawn(&self.world, kind, Team::Red, h).is_some() {
                return;
            }
        }
    }
}

impl Scene for RealtimeSingle {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let dt = ctx.time.dt;
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        self.hover = if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)) };
        realtime::time_controls(ctx, &mut self.rt, &mut self.resume_speed);

        let alive = self.rt.units[HERO].alive();
        if alive {
            if ctx.input.clicked(MouseButton::Left)
                && !over_ui
                && let Some(p) = self.hover
            {
                match self.rt.unit_at(p.hex).filter(|&u| self.rt.units[u].team() == Team::Red) {
                    Some(m) => self.rt.order_attack(&[HERO], m),
                    None => self.rt.order_move(&self.world, &[HERO], p.hex),
                }
            }
            if (ctx.input.key_pressed(Key::Q) || ctx.input.ui_clicked("nova")) && self.nova <= 0.0 {
                self.nova = NOVA_COOLDOWN;
                ctx.audio.play(self.art.sfx.magic);
                let events = self.rt.blast(HERO, 2, (10, 16));
                let p = self.rt.units[HERO].actor.pos + Vec3::Y * 0.5;
                self.fx.burst(
                    p,
                    &Burst {
                        count: 70,
                        speed: (3.0, 6.0),
                        spread: 1.0,
                        lift: 0.5,
                        ..Burst::sparks(Color::hex(0x9fd8ff))
                    },
                );
                self.kills += events.len() as u32;
            }
        } else if ctx.input.ui_clicked("respawn") || ctx.input.key_pressed(Key::Enter) {
            *self = RealtimeSingle::new(self.art.clone());
            return Transition::None;
        }

        let sim_dt = dt * self.rt.speed;
        for e in self.rt.step(&self.world, &self.art, dt) {
            match e {
                RtEvent::Killed { by } if by == HERO => self.kills += 1,
                RtEvent::Struck { pos, ranged } => common::strike_sound(ctx, &self.art, pos, ranged, &self.camera),
                _ => {}
            }
        }
        self.nova = (self.nova - sim_dt).max(0.0);
        self.fx.update(sim_dt);
        if sim_dt > 0.0 {
            let h = &mut self.rt.units[HERO].actor;
            h.hp = (h.hp + sim_dt * 1.0).min(h.max_hp);
            self.spawn_timer -= sim_dt;
            let living = self.rt.units.iter().filter(|u| u.team() == Team::Red && u.alive()).count();
            if self.spawn_timer <= 0.0 && living < 12 {
                self.spawn_timer = 3.0;
                self.spawn();
            }
        }
        self.camera.move_to(self.rt.units[HERO].actor.pos);
        let hero = &self.rt.units[HERO];
        ctx.status = format!(
            "realtime-single hp={:.0} kills={} hex={},{} moving={} speed={} monsters={}",
            hero.actor.hp.max(0.0),
            self.kills,
            hero.hex().q,
            hero.hex().r,
            hero.actor.moving(),
            self.rt.speed,
            self.rt.units.iter().filter(|u| u.team() == Team::Red && u.alive()).count()
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let hero = &self.rt.units[HERO];
        let route: Vec<Hex> = std::iter::once(hero.actor.mover.step().map_or(hero.hex(), |s| s.0))
            .chain(hero.actor.mover.route())
            .collect();
        let target = match hero.order {
            Order::Attack(t) => Some(t),
            _ => None,
        };
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
            w.set_on_top(true);
            if route.len() > 1 {
                w.path(&route, 0.07, Color::WHITE.with_alpha(0.7), false);
                w.hex_outline(*route.last().unwrap(), Color::WHITE.with_alpha(0.9), 0.08);
            }
            if let Some(t) = target {
                w.hex_outline(self.rt.units[t].hex(), Color::RED, 0.12);
            }
            if let Some(p) = self.hover {
                w.hex_outline(p.hex, Color::hex(0xffe27a).with_alpha(0.7), 0.06);
            }
            w.set_on_top(false);
            for u in &self.rt.units {
                u.actor.draw(&mut w, &self.art, u.team() == Team::Red);
            }
            self.rt.draw_fx(&mut w, &self.art);
            self.fx.draw(&mut w);
        }
        common::header(
            f,
            "Realtime - Single Unit",
            "One hero in real time, still hex by hex; terrain sets the walking speed",
        );
        common::stats(f);
        common::help(
            f,
            &[
                "Click a hex: walk   Click a monster: chase and attack   Q: frost nova",
                "Road x2 speed, forest /2, snow /2.5, uphill slower   Right drag: pan",
                "Space: pause (orders still work)   1 / 2 / 3: speed 1x / 2x / 4x",
            ],
        );
        let size = f.ui_size();
        realtime::draw_time_controls(f, &self.rt, Vec2::new(size.x - 208.0, 106.0));

        // Terrain under the cursor and what it does to speed.
        if let Some(p) = self.hover {
            let text = match self.world.tile(p.hex) {
                Some(t) if !self.world.is_submerged(p.hex) => {
                    let m = &self.world.materials[t.material as usize];
                    format!("{}: {:.0}% speed", m.name, 100.0 / m.move_cost)
                }
                _ => "water: impassable".to_string(),
            };
            f.tooltip(&text);
        }

        let t = f.theme.clone();
        let hero = &self.rt.units[HERO];
        let bar = Rect::new(size.x - 468.0, size.y - 76.0, 460.0, 68.0);
        f.panel(bar);
        f.text(Vec2::new(bar.x + 12.0, bar.y + 10.0), "HERO", 8.0, t.text_dim);
        f.bar(
            Rect::new(bar.x + 12.0, bar.y + 24.0, 180.0, 12.0),
            hero.actor.hp / hero.actor.max_hp,
            Color::hex(0xd8302a),
            Color::hex(0x201010),
        );
        f.text(
            Vec2::new(bar.x + 12.0, bar.y + 44.0),
            &format!("{:.0} hp   kills {}", hero.actor.hp.max(0.0), self.kills),
            8.0,
            t.text,
        );
        let slot = Rect::new(bar.x + 200.0, bar.y + 8.0, 52.0, 52.0);
        f.button_ex("nova", slot, "", self.nova <= 0.0, false);
        f.image_fit(
            slot.inset(8.0),
            self.art.bolt,
            if self.nova <= 0.0 { Color::WHITE } else { Color::rgb(0.3, 0.3, 0.3) },
        );
        if self.nova > 0.0 {
            f.text_centered(slot, &format!("{:.0}", self.nova.ceil()), 16.0, Color::WHITE);
        }
        f.text(Vec2::new(slot.x + 60.0, slot.y + 8.0), "Q  Frost nova", 8.0, t.text);
        f.text(Vec2::new(slot.x + 60.0, slot.y + 22.0), "all within 2 hexes", 8.0, t.text_dim);
        if !hero.alive() {
            let r = Rect::new(size.x * 0.5 - 160.0, size.y * 0.5 - 70.0, 320.0, 140.0);
            f.panel(r);
            f.text_centered(Rect::new(r.x, r.y + 20.0, r.w, 30.0), "YOU DIED", 24.0, Color::RED);
            f.button("respawn", Rect::new(r.x + 80.0, r.y + 80.0, 160.0, 36.0), "Try again");
        }
    }
}
