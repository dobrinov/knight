//! Overlays, light & FX: territory borders (region outlines), a river on hex edges, a day/night
//! cycle through the world's ambient light, and particles (chimney smoke, torches after dark,
//! fireworks where you click).

use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path;
use knight_engine::hex::{Hex, HexMap, HexSet, Rng};
use knight_engine::*;

use crate::art::Art;
use crate::common::{self, Decor, MapGen};

const REALMS: [(&str, u32); 4] =
    [("Azure", 0x3d7be0), ("Crimson", 0xd8453c), ("Verdant", 0x4caf50), ("Amber", 0xe0a030)];

pub struct OverlaysDemo {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    camera: Camera,
    controller: CameraController,
    owner: HexMap<usize>,
    regions: Vec<HexSet>,
    capitals: Vec<Hex>,
    villages: Vec<Hex>,
    river: Vec<(Hex, usize)>,
    fx: Particles,
    rng: Rng,
    /// Time of day in hours 0..24.
    hour: f32,
    auto_time: bool,
    show_borders: bool,
    hover: Option<Hex>,
    smoke_t: f32,
}

impl OverlaysDemo {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 64, size: 22, amplitude: 7.0, island: 0.7, trees: 0.4, ..Default::default() },
        );
        let mut rng = Rng::new(3);
        let mut land: Vec<Hex> =
            world.hexes().filter(|&h| world.tile(h).is_some_and(|t| (1..=3).contains(&t.height))).collect();
        land.sort();
        // Capitals far apart; territories grow from them over walkable land (multi-source Dijkstra).
        let mut capitals: Vec<Hex> = Vec::new();
        for _ in 0..REALMS.len() {
            let pick = (0..80)
                .map(|_| land[rng.range(0, land.len() as i32) as usize])
                .max_by_key(|h| capitals.iter().map(|c| c.distance(*h)).min().unwrap_or(99).min(14))
                .unwrap();
            capitals.push(pick);
        }
        let mut owner: HexMap<usize> = HexMap::new();
        let mut best: HexMap<u32> = HexMap::new();
        for (i, &c) in capitals.iter().enumerate() {
            let r = path::reachable(c, 60, |a, b| world.step_points(a, b, 2, 4));
            for (h, &(_, cost)) in r.nodes.iter() {
                if best.get(h).is_none_or(|&b| cost < b) {
                    best.insert(h, cost);
                    owner.insert(h, i);
                }
            }
        }
        let regions: Vec<HexSet> =
            (0..REALMS.len()).map(|i| owner.iter().filter(|(_, o)| **o == i).map(|(h, _)| h).collect()).collect();
        let villages: Vec<Hex> = capitals
            .iter()
            .flat_map(|&c| {
                let mut v: Vec<Hex> = c.ring(3).into_iter().filter(|h| land.contains(h)).collect();
                v.truncate(2);
                v
            })
            .collect();
        let mut decor = decor;
        decor.retain(|d| capitals.iter().chain(&villages).all(|c| c.distance(d.hex) > 1));
        // A river along hex edges: the boundary between the hexes north and south of a wavy
        // line is one unbroken chain of edges.
        let south = |h: Hex| {
            let p = world.layout.hex_to_point(h);
            p.y > 3.0 + (p.x * 0.18).sin() * 3.5 + (p.x * 0.05).cos() * 2.0
        };
        let mut river = Vec::new();
        for h in world.hexes() {
            if south(h) || world.is_submerged(h) {
                continue;
            }
            for d in 0..6 {
                let n = h.neighbor(d);
                if south(n) && world.contains(n) && !world.is_submerged(n) {
                    river.push((h, d));
                }
            }
        }
        let camera = Camera::new(world.center(), 30.0).with_pitch(52.0);
        OverlaysDemo {
            art,
            world,
            decor,
            camera,
            controller: CameraController::default(),
            owner,
            regions,
            capitals,
            villages,
            river,
            fx: Particles::new(),
            rng,
            hour: 12.0,
            auto_time: true,
            show_borders: true,
            hover: None,
            smoke_t: 0.0,
        }
    }

    /// Ambient light for the hour: warm dawn and dusk, blue night.
    fn ambient(hour: f32) -> Color {
        let keys: [(f32, u32); 7] = [
            (0.0, 0x2a3570),
            (5.0, 0x3a4080),
            (7.0, 0xffb080),
            (12.0, 0xffffff),
            (18.0, 0xffc890),
            (20.0, 0x7060a0),
            (24.0, 0x2a3570),
        ];
        let h = hour.rem_euclid(24.0);
        let k = keys.windows(2).find(|w| h >= w[0].0 && h <= w[1].0).unwrap();
        let t = (h - k[0].0) / (k[1].0 - k[0].0);
        Color::hex(k[0].1).lerp(Color::hex(k[1].1), t)
    }

    fn is_night(&self) -> bool {
        !(6.5..19.5).contains(&self.hour)
    }
}

impl Scene for OverlaysDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let dt = ctx.time.dt;
        let input = &ctx.input;
        for (id, h) in [("dawn", 6.5), ("noon", 12.0), ("dusk", 19.0), ("night", 23.0)] {
            if input.ui_clicked(id) {
                self.hour = h;
                self.auto_time = false;
            }
        }
        if input.ui_clicked("auto") || input.key_pressed(Key::T) {
            self.auto_time = !self.auto_time;
        }
        if input.ui_clicked("borders") || input.key_pressed(Key::B) {
            self.show_borders = !self.show_borders;
        }
        if self.auto_time {
            self.hour = (self.hour + dt * 1.2).rem_euclid(24.0);
        }
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        let pick = if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)) };
        self.hover = pick.map(|p| p.hex);
        if ctx.input.clicked(MouseButton::Left)
            && let Some(p) = pick
        {
            // Fireworks: a burst of sparks in a random realm colour, high above the click.
            let col = Color::hex(REALMS[self.rng.range(0, 4) as usize].1).lerp(Color::WHITE, 0.3);
            self.fx.burst(
                p.point + Vec3::Y * 3.0,
                &Burst { count: 90, speed: (2.0, 5.0), lift: 0.0, ..Burst::sparks(col) },
            );
            self.fx.burst(p.point, &Burst { count: 12, ..Burst::smoke() });
            ctx.audio.play_at(self.art.sfx.explosion, p.point, &self.camera);
        }
        // Chimney smoke by day, torches by night.
        self.smoke_t -= dt;
        if self.smoke_t <= 0.0 {
            self.smoke_t = 0.12;
            let night = self.is_night();
            for &v in self.villages.iter().chain(&self.capitals) {
                let p = self.world.hex_to_world(v);
                if night {
                    for dx in [-0.45f32, 0.45] {
                        self.fx.burst(p + Vec3::new(dx, 0.9, 0.3), &Burst { count: 2, ..Burst::fire() });
                    }
                } else if self.rng.chance(0.3) {
                    self.fx.burst(p + Vec3::new(0.2, 1.6, 0.0), &Burst { count: 1, ..Burst::smoke() });
                }
            }
        }
        self.fx.update(dt);
        ctx.status = format!(
            "overlays hour={:.1} night={} particles={} realms={}",
            self.hour,
            self.is_night(),
            self.fx.len(),
            self.regions.iter().filter(|r| !r.is_empty()).count()
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let ambient = Self::ambient(self.hour);
        {
            let mut w = f.world(&mut self.world, &self.camera);
            w.set_ambient(ambient);
            for d in &self.decor {
                d.draw(&mut w);
            }
            for &(h, dir) in &self.river {
                w.hex_edge(h, dir, Color::hex(0x4a90e0).with_alpha(0.95), 0.18);
            }
            if self.show_borders {
                for (i, r) in self.regions.iter().enumerate() {
                    let col = Color::hex(REALMS[i].1);
                    for &h in r {
                        w.hex_fill(h, col.with_alpha(0.12));
                    }
                    w.region_outline(r, col, 0.1);
                }
            }
            if let Some(h) = self.hover
                && let Some(&o) = self.owner.get(h)
            {
                w.set_on_top(true);
                w.region_outline(&self.regions[o], Color::WHITE.with_alpha(0.7), 0.05);
                w.set_on_top(false);
            }
            for (i, &c) in self.capitals.iter().enumerate() {
                let p = w.world.hex_to_world(c);
                w.sprite(Sprite::new(self.art.keep_neutral, p).scale(1.2));
                w.label(p + Vec3::Z * 0.8, REALMS[i].0, 8.0, Color::hex(REALMS[i].1).lerp(Color::WHITE, 0.4));
            }
            for &v in &self.villages {
                w.sprite(Sprite::new(self.art.village_neutral, w.world.hex_to_world(v)).scale(0.9));
            }
            self.fx.draw(&mut w);
        }
        common::header(f, "Overlays, Light & FX", "Region borders, hex-edge rivers, day/night lighting and particles");
        common::stats(f);
        common::help(
            f,
            &["Click: fireworks   Hover: highlight a realm", "B: borders   T: auto time   Drag: pan   Wheel: zoom"],
        );
        let size = f.ui_size();
        let r = Rect::new(size.x - 208.0, 106.0, 200.0, 126.0);
        f.panel(r);
        let t = f.theme.clone();
        let hh = self.hour as u32;
        let mm = ((self.hour.fract()) * 60.0) as u32;
        f.text(Vec2::new(r.x + 10.0, r.y + 10.0), &format!("TIME {hh:02}:{mm:02}"), 8.0, t.accent);
        let c = Rect::new(r.x + 8.0, r.y + 24.0, r.w - 16.0, 22.0).cols(4, 3.0);
        f.button("dawn", c[0], "Dawn");
        f.button("noon", c[1], "Noon");
        f.button("dusk", c[2], "Dusk");
        f.button("night", c[3], "Night");
        f.button_ex("auto", Rect::new(r.x + 8.0, r.y + 52.0, r.w - 16.0, 22.0), "Day cycle [T]", true, self.auto_time);
        f.button_ex(
            "borders",
            Rect::new(r.x + 8.0, r.y + 80.0, r.w - 16.0, 22.0),
            "Borders [B]",
            true,
            self.show_borders,
        );
        f.text(Vec2::new(r.x + 10.0, r.y + 108.0), &format!("{} particles", self.fx.len()), 8.0, t.text_dim);
        if let Some(h) = self.hover
            && let Some(&o) = self.owner.get(h)
        {
            f.tooltip(&format!("Realm of {} ({} hexes)", REALMS[o].0, self.regions[o].len()));
        }
    }
}
