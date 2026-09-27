//! The adventure map: move the hero with daily movement points, collect resources, capture mines,
//! visit the town (pushes the town scene) and fight neutral stacks (pushes the battle scene).

use std::any::Any;
use std::collections::VecDeque;
use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path;
use knight_engine::hex::{Hex, Rng};
use knight_engine::*;

use super::battle::BattleScene;
use super::town::TownScene;
use super::{BattleResult, Purse, Stack, TownResult};
use crate::art::{Art, Mats, Team, UnitKind};
use crate::common::{self, Actor, Decor, MapGen};

const MAX_MP: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Obj {
    Gold(u32),
    Wood(u32),
    Ore(u32),
    Sawmill(bool),
    OreMine(bool),
    Monster(Stack),
}

struct Floater {
    pos: Vec3,
    text: String,
    color: Color,
    t: f32,
}

pub struct MapScene {
    art: Rc<Art>,
    world: HexWorld,
    mats: Mats,
    decor: Vec<Decor>,
    camera: Camera,
    controller: CameraController,
    hero: Actor,
    hero_hex: Hex,
    mp: u32,
    day: u32,
    purse: Purse,
    army: Vec<Stack>,
    built: [bool; 5],
    town: Hex,
    objects: Vec<(Hex, Obj)>,
    planned: Option<(Hex, Vec<Hex>)>,
    walking: VecDeque<Hex>,
    floaters: Vec<Floater>,
    hover: Option<Hex>,
    fighting: Option<usize>,
    battles_won: u32,
    sounds: Vec<(SoundId, Vec3)>,
}

/// Movement points for a step: the world's terrain cost at 2 points per normal step (road 1,
/// grass 2, sand 3, forest 4, snow 5), no water, at most one level of climb.
fn terrain_cost(world: &HexWorld, _mats: &Mats, from: Hex, to: Hex) -> Option<u32> {
    world.step_points(from, to, 1, 2)
}

impl MapScene {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 99, size: 20, amplitude: 7.0, island: 0.75, trees: 0.6, ..Default::default() },
        );
        let land: Vec<Hex> = {
            let mut v: Vec<Hex> =
                world.hexes().filter(|&h| world.tile(h).is_some_and(|t| t.height >= 1 && t.height <= 4)).collect();
            v.sort();
            v
        };
        let town = *land.iter().min_by_key(|h| h.distance(Hex::new(-4, 2))).unwrap_or(&Hex::ORIGIN);
        world.set_material(town, mats.road);
        let mut decor = decor;
        decor.retain(|d| d.hex.distance(town) > 1);

        // Scatter objects on reachable land.
        let mut rng = Rng::new(1234);
        let mut objects: Vec<(Hex, Obj)> = Vec::new();
        let free = |rng: &mut Rng, objects: &Vec<(Hex, Obj)>, min: i32, max: i32| -> Option<Hex> {
            for _ in 0..200 {
                let h = land[rng.range(0, land.len() as i32) as usize];
                let d = h.distance(town);
                if d >= min && d <= max && objects.iter().all(|(o, _)| o.distance(h) > 1) {
                    return Some(h);
                }
            }
            None
        };
        for (obj, n, min, max) in [
            (Obj::Gold(500), 5, 2, 14),
            (Obj::Wood(5), 3, 2, 14),
            (Obj::Ore(5), 3, 2, 14),
            (Obj::Sawmill(false), 1, 4, 10),
            (Obj::OreMine(false), 1, 4, 12),
            (Obj::Monster(Stack::new(UnitKind::Wolf, 6)), 1, 3, 6),
            (Obj::Monster(Stack::new(UnitKind::Footman, 12)), 1, 5, 9),
            (Obj::Monster(Stack::new(UnitKind::Wolf, 14)), 1, 7, 12),
            (Obj::Monster(Stack::new(UnitKind::Ogre, 8)), 1, 8, 14),
            (Obj::Monster(Stack::new(UnitKind::Griffon, 2)), 1, 10, 18),
        ] {
            for _ in 0..n {
                if let Some(h) = free(&mut rng, &objects, min, max) {
                    objects.push((h, obj));
                }
            }
        }
        decor.retain(|d| objects.iter().all(|(h, _)| *h != d.hex));

        let hero_hex =
            town.neighbors().into_iter().find(|&n| terrain_cost(&world, &mats, town, n).is_some()).unwrap_or(town);
        let mut hero = Actor::new(UnitKind::Knight, Team::Blue, &world, hero_hex);
        hero.mover.speed = 2.2;
        let camera = Camera::new(world.hex_to_world(hero_hex), 44.0).with_pitch(48.0);
        MapScene {
            art,
            world,
            mats,
            decor,
            camera,
            controller: CameraController::default(),
            hero,
            hero_hex,
            mp: MAX_MP,
            day: 1,
            purse: Purse::new(2500, 10, 10),
            army: vec![Stack::new(UnitKind::Footman, 14), Stack::new(UnitKind::Archer, 8)],
            built: [true, false, false, false, false],
            town,
            objects,
            planned: None,
            walking: VecDeque::new(),
            floaters: Vec::new(),
            hover: None,
            fighting: None,
            battles_won: 0,
            sounds: Vec::new(),
        }
    }

    fn monster_at(&self, h: Hex) -> Option<usize> {
        self.objects.iter().position(|(o, obj)| *o == h && matches!(obj, Obj::Monster(_)))
    }

    fn plan(&self, goal: Hex) -> Option<Vec<Hex>> {
        let (world, mats) = (&self.world, &self.mats);
        let monsters: Vec<Hex> =
            self.objects.iter().filter(|(_, o)| matches!(o, Obj::Monster(_))).map(|(h, _)| *h).collect();
        path::astar(self.hero_hex, goal, 1, u32::MAX, |a, b| {
            if b != goal && monsters.contains(&b) {
                return None;
            }
            terrain_cost(world, mats, a, b)
        })
        .map(|p| p.hexes)
    }

    fn float(&mut self, h: Hex, text: String, color: Color) {
        let sfx = if color == Color::YELLOW { self.art.sfx.coin } else { self.art.sfx.powerup };
        self.sounds.push((sfx, self.world.hex_to_world(h)));
        self.floaters.push(Floater { pos: self.world.hex_to_world(h) + Vec3::Y * 1.2, text, color, t: 0.0 });
    }

    fn end_turn(&mut self) {
        self.day += 1;
        self.mp = MAX_MP;
        let mut income = Purse::new(750, 0, 0);
        for (_, o) in &self.objects {
            match o {
                Obj::Sawmill(true) => income.wood += 2,
                Obj::OreMine(true) => income.ore += 2,
                _ => {}
            }
        }
        self.purse.gold += income.gold;
        self.purse.wood += income.wood;
        self.purse.ore += income.ore;
        let text = format!(
            "Day {}: +{} gold{}{}",
            self.day,
            income.gold,
            if income.wood > 0 { ", +2 wood" } else { "" },
            if income.ore > 0 { ", +2 ore" } else { "" }
        );
        self.float(self.hero_hex, text, Color::YELLOW);
        if let Some((goal, _)) = self.planned {
            self.planned = self.plan(goal).map(|p| (goal, p));
        }
    }

    /// Hero finished stepping onto `h`.
    fn arrive(&mut self, h: Hex) -> Transition {
        self.hero_hex = h;
        if let Some(i) = self.objects.iter().position(|(o, _)| *o == h) {
            match self.objects[i].1 {
                Obj::Gold(n) => {
                    self.purse.gold += n;
                    self.float(h, format!("+{n} gold"), Color::YELLOW);
                    self.objects.remove(i);
                }
                Obj::Wood(n) => {
                    self.purse.wood += n;
                    self.float(h, format!("+{n} wood"), Color::hex(0xd8a060));
                    self.objects.remove(i);
                }
                Obj::Ore(n) => {
                    self.purse.ore += n;
                    self.float(h, format!("+{n} ore"), Color::hex(0xb0b8c8));
                    self.objects.remove(i);
                }
                Obj::Sawmill(false) => {
                    self.objects[i].1 = Obj::Sawmill(true);
                    self.float(h, "Sawmill captured: +2 wood/day".into(), Color::GREEN);
                }
                Obj::OreMine(false) => {
                    self.objects[i].1 = Obj::OreMine(true);
                    self.float(h, "Ore mine captured: +2 ore/day".into(), Color::GREEN);
                }
                _ => {}
            }
        }
        if h == self.town {
            self.walking.clear();
            return self.open_town();
        }
        Transition::None
    }

    fn open_town(&self) -> Transition {
        Transition::Push(Box::new(TownScene::new(self.art.clone(), self.purse, self.army.clone(), self.built)))
    }
}

impl Scene for MapScene {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        common::music(ctx, self.art.sfx.music);
        common::flush_sounds(ctx, &mut self.sounds, &self.camera);
        let dt = ctx.time.dt;
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        self.hover =
            if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)).map(|p| p.hex) };

        if ctx.input.ui_clicked("endturn") || ctx.input.key_pressed(Key::Enter) {
            self.end_turn();
        }
        if ctx.input.ui_clicked("town") || ctx.input.key_pressed(Key::T) {
            return self.open_town();
        }

        // Heroes-style orders: first click plans, second click on the same hex goes.
        if ctx.input.clicked(MouseButton::Left)
            && !over_ui
            && self.walking.is_empty()
            && let Some(h) = self.hover
        {
            match &self.planned {
                Some((goal, p)) if *goal == h => {
                    self.walking = p.iter().skip(1).copied().collect();
                }
                _ => self.planned = self.plan(h).map(|p| (h, p)),
            }
        }

        // Step along the route hex by hex, paying movement points.
        let mut transition = Transition::None;
        if !self.hero.moving()
            && let Some(&next) = self.walking.front()
        {
            if let Some(i) = self.monster_at(next) {
                self.walking.clear();
                self.planned = None;
                if let Obj::Monster(enemy) = self.objects[i].1 {
                    self.fighting = Some(i);
                    self.hero.facing_left = self.world.hex_to_world(next).x < self.hero.pos.x;
                    return Transition::Push(Box::new(BattleScene::new(
                        self.art.clone(),
                        self.army.clone(),
                        vec![enemy],
                    )));
                }
            }
            let cost = terrain_cost(&self.world, &self.mats, self.hero_hex, next).unwrap_or(99);
            if cost <= self.mp {
                self.mp -= cost;
                self.walking.pop_front();
                self.hero.walk_hexes(&[next]);
                self.hero_hex = next;
                if self.walking.is_empty() {
                    self.planned = None;
                } else if let Some((_, p)) = &mut self.planned
                    && !p.is_empty()
                {
                    p.remove(0);
                }
            } else {
                self.walking.clear();
            }
        }
        let was_moving = self.hero.moving();
        self.hero.update(&self.world, &self.art, dt);
        if was_moving && !self.hero.moving() {
            transition = self.arrive(self.hero_hex);
        }
        if self.hero.moving() {
            self.camera.move_to(self.hero.pos);
        }
        for f in &mut self.floaters {
            f.t += dt;
            f.pos.y += dt * 0.6;
        }
        self.floaters.retain(|f| f.t < 2.2);
        ctx.status = format!(
            "heroes-map day={} mp={} gold={} wood={} ore={} army={} objects={} won={}",
            self.day,
            self.mp,
            self.purse.gold,
            self.purse.wood,
            self.purse.ore,
            self.army.iter().map(|s| format!("{}x{}", s.count, s.kind.name())).collect::<Vec<_>>().join("+"),
            self.objects.len(),
            self.battles_won
        );
        transition
    }

    fn on_resume(&mut self, ctx: &mut Context, result: Option<Box<dyn Any>>) {
        let Some(result) = result else { return };
        let result = match result.downcast::<TownResult>() {
            Ok(t) => {
                self.purse = t.purse;
                self.army = t.army;
                self.built = t.built;
                return;
            }
            Err(r) => r,
        };
        if let Ok(b) = result.downcast::<BattleResult>() {
            self.army = b.army;
            if let Some(i) = self.fighting.take() {
                let h = self.objects[i].0;
                if b.won {
                    self.objects.remove(i);
                    self.battles_won += 1;
                    self.purse.gold += 300;
                    self.float(h, "Victory! +300 gold".into(), Color::YELLOW);
                } else {
                    self.float(self.hero_hex, "Defeat - the hero retreats to town".into(), Color::RED);
                    self.hero_hex = self.town;
                    self.hero.place(&self.world, self.town);
                    self.army = vec![Stack::new(UnitKind::Footman, 5)];
                }
            }
        }
        let _ = ctx;
    }

    fn draw(&mut self, f: &mut Frame) {
        let mp = self.mp;
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
            // Town.
            let tp = w.world.hex_to_world(self.town);
            w.shadow(tp, 0.8, 0.3);
            w.sprite(Sprite::new(self.art.keep[0], tp).scale(1.25));
            w.label(tp + Vec3::Z * 0.75, "Knightsbridge", 8.0, Color::hex(0xbfd8ff));
            for (h, o) in &self.objects {
                let p = w.world.hex_to_world(*h);
                match o {
                    Obj::Gold(_) => {
                        w.sprite(Sprite::new(self.art.treasure, p).scale(0.9));
                    }
                    Obj::Wood(_) => {
                        w.sprite(Sprite::new(self.art.wood, p).scale(0.8));
                    }
                    Obj::Ore(_) => {
                        w.sprite(Sprite::new(self.art.ore, p).scale(0.8));
                    }
                    Obj::Sawmill(owned) | Obj::OreMine(owned) => {
                        let img = if matches!(o, Obj::Sawmill(_)) { self.art.sawmill } else { self.art.ore_mine };
                        w.sprite(Sprite::new(img, p));
                        if *owned {
                            w.sprite(Sprite::new(self.art.flag[0], p + Vec3::new(0.6, 0.0, 0.1)).bias(0.2));
                        }
                    }
                    Obj::Monster(s) => {
                        let art = self.art.unit(s.kind, Team::Red);
                        let t = w.time() + h.q as f32 * 0.37;
                        w.shadow(p, 0.4, 0.35);
                        w.sprite(Sprite::new(art.idle.frame_at(t), p).flip(true));
                        w.label(p + Vec3::Y * 0.1 + Vec3::Z * 0.45, &format!("{}", s.count), 8.0, Color::hex(0xffd0c0));
                    }
                }
            }
            // Planned route: green while affordable today, red beyond. Drawn on top so hills
            // and forests never hide it.
            w.set_on_top(true);
            if let Some((_, p)) = &self.planned {
                let mut left = mp as i64;
                for pair in p.windows(2) {
                    left -= terrain_cost(w.world, &self.mats, pair[0], pair[1]).unwrap_or(1) as i64;
                    let col = if left >= 0 { Color::hex(0x6ef08a) } else { Color::hex(0xff6a5a) };
                    let (a, b) = (w.world.hex_to_world(pair[0]), w.world.hex_to_world(pair[1]));
                    w.line(a, b, 0.09, col.with_alpha(0.75));
                    w.ring(b, 0.22, col);
                }
                if let Some(&last) = p.last() {
                    w.hex_outline(last, Color::WHITE, 0.1);
                }
            } else if let Some(h) = self.hover {
                w.hex_outline(h, Color::WHITE.with_alpha(0.6), 0.08);
            }
            w.set_on_top(false);
            self.hero.draw(&mut w, &self.art, false);
            w.sprite(
                Sprite::new(self.art.flag[0], self.hero.pos + Vec3::new(-0.35, 0.0, -0.05)).scale(0.7).bias(-0.05),
            );
            for fl in &self.floaters {
                let a = (2.2 - fl.t).clamp(0.0, 1.0);
                w.label(fl.pos, &fl.text, 8.0, fl.color.with_alpha(a));
            }
        }
        // Resource bar.
        let size = f.ui_size();
        let bar = Rect::new(size.x * 0.5 - 250.0, 8.0, 500.0, 36.0);
        f.panel(bar);
        let cols = bar.inset(4.0).cols(4, 4.0);
        let t = f.theme.clone();
        for (i, (img, text)) in [
            (Some(self.art.gold), format!("{}", self.purse.gold)),
            (Some(self.art.wood), format!("{}", self.purse.wood)),
            (Some(self.art.ore), format!("{}", self.purse.ore)),
            (None, format!("Day {}", self.day)),
        ]
        .into_iter()
        .enumerate()
        {
            let c = cols[i];
            let mut x = c.x + 6.0;
            if let Some(img) = img {
                f.image(Rect::new(c.x, c.y, 28.0, 28.0), img, Color::WHITE);
                x += 26.0;
            }
            f.text(Vec2::new(x, c.y + 6.0), &text, 16.0, t.text);
        }
        common::header(f, "Scenes & UI Levels", "Map scene -> town scene -> battle scene, results passed back down");
        common::stats(f);

        // Hero panel.
        let r = Rect::new(size.x - 208.0, 106.0, 200.0, 290.0);
        f.panel(r);
        f.text(Vec2::new(r.x + 10.0, r.y + 10.0), "SIR ROLAND", 8.0, t.accent);
        f.text(Vec2::new(r.x + 10.0, r.y + 26.0), &format!("Movement {mp}/{MAX_MP}"), 8.0, t.text_dim);
        f.bar(
            Rect::new(r.x + 10.0, r.y + 40.0, r.w - 20.0, 8.0),
            mp as f32 / MAX_MP as f32,
            Color::hex(0x6ef08a),
            Color::hex(0x202020),
        );
        f.text(Vec2::new(r.x + 10.0, r.y + 58.0), "ARMY", 8.0, t.text_dim);
        for (i, s) in self.army.iter().enumerate() {
            let cell = Rect::new(r.x + 8.0 + (i % 4) as f32 * 46.0, r.y + 72.0 + (i / 4) as f32 * 56.0, 42.0, 50.0);
            f.rect(cell, Color::hex(0x2a2432));
            f.outline(cell, t.panel_border, 1.0);
            let img = self.art.unit(s.kind, Team::Blue).idle.frames[0];
            f.image_fit(Rect::new(cell.x, cell.y - 4.0, cell.w, cell.h), img, Color::WHITE);
            f.text(Vec2::new(cell.x + 3.0, cell.y + cell.h - 11.0), &format!("{}", s.count), 8.0, Color::WHITE);
        }
        f.button("town", Rect::new(r.x + 8.0, r.y + 190.0, r.w - 16.0, 30.0), "Town  [T]");
        f.button("endturn", Rect::new(r.x + 8.0, r.y + 226.0, r.w - 16.0, 30.0), "End day  [Enter]");
        f.text(Vec2::new(r.x + 10.0, r.y + 266.0), &format!("Battles won: {}", self.battles_won), 8.0, t.text_dim);
        common::help(
            f,
            &[
                "Click a hex: plan a route (green = reachable today). Click it again: go.",
                "Walk into chests, wood, ore, mines. Walk into monsters to fight.",
                "Visit the castle to build and recruit.   Drag: pan   Wheel: zoom",
            ],
        );
    }
}
