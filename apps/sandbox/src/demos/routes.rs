//! Routes & transport: survey and build railways between towns (A* over a construction-cost
//! field), run trains along them and earn money. Rails are custom geometry following the terrain;
//! steps between levels get small ramps.

use std::collections::HashSet;
use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path;
use knight_engine::hex::{Hex, Rng};
use knight_engine::mesh::Vertex;
use knight_engine::*;

use crate::art::Art;
use crate::common::{self, Decor, MapGen};

const NAMES: [&str; 8] =
    ["Ashford", "Brightwater", "Coldharbour", "Dunmore", "Eastwick", "Fairhaven", "Greystone", "Highmoor"];

struct Town {
    hex: Hex,
    name: &'static str,
    houses: Vec<(Vec2, f32)>,
}

struct Train {
    s: f32,
    dir: f32,
    speed: f32,
    wait: f32,
}

struct Route {
    a: usize,
    b: usize,
    hexes: Vec<Hex>,
    points: Vec<Vec3>,
    lengths: Vec<f32>,
    trains: Vec<Train>,
    earned: u32,
}

impl Route {
    fn total(&self) -> f32 {
        *self.lengths.last().unwrap_or(&0.0)
    }

    /// Position and direction at distance `s` along the line.
    fn at(&self, s: f32) -> (Vec3, Vec3) {
        let s = s.clamp(0.0, self.total());
        let i = self.lengths.partition_point(|&l| l < s).clamp(1, self.points.len() - 1);
        let (a, b) = (self.points[i - 1], self.points[i]);
        let seg = self.lengths[i] - self.lengths[i - 1];
        let t = if seg > 0.0 { (s - self.lengths[i - 1]) / seg } else { 0.0 };
        (a.lerp(b, t), (b - a).normalize_or_zero())
    }
}

struct Floater {
    pos: Vec3,
    text: String,
    t: f32,
}

pub struct RoutesDemo {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    camera: Camera,
    controller: CameraController,
    towns: Vec<Town>,
    routes: Vec<Route>,
    track: HashSet<(Hex, Hex)>,
    from: Option<usize>,
    hover: Option<Hex>,
    preview: Option<(usize, Vec<Hex>, u32)>,
    money: i64,
    day: f32,
    floaters: Vec<Floater>,
    message: String,
}

fn edge(a: Hex, b: Hex) -> (Hex, Hex) {
    if a < b { (a, b) } else { (b, a) }
}

impl RoutesDemo {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 31, size: 26, amplitude: 7.0, island: 0.7, trees: 0.35, ..Default::default() },
        );
        let mut rng = Rng::new(8);
        let mut land: Vec<Hex> =
            world.hexes().filter(|&h| world.tile(h).is_some_and(|t| (1..=3).contains(&t.height))).collect();
        land.sort();
        let mut towns: Vec<Town> = Vec::new();
        for name in NAMES {
            // Farthest-point sampling keeps towns apart.
            let pick = (0..60)
                .map(|_| land[rng.range(0, land.len() as i32) as usize])
                .max_by_key(|h| towns.iter().map(|t| t.hex.distance(*h)).min().unwrap_or(99).min(12));
            if let Some(h) = pick
                && towns.iter().all(|t| t.hex.distance(h) >= 7)
            {
                let houses = (0..3)
                    .map(|_| (Vec2::new(rng.range_f32(-0.9, 0.9), rng.range_f32(-0.6, 0.6)), rng.range_f32(0.55, 0.75)))
                    .collect();
                towns.push(Town { hex: h, name, houses });
            }
        }
        for t in &towns {
            for h in t.hex.spiral(1) {
                if world.contains(h) && !world.is_submerged(h) {
                    world.set_material(h, mats.road);
                }
            }
        }
        let mut decor = decor;
        decor.retain(|d| towns.iter().all(|t| t.hex.distance(d.hex) > 1));
        let camera = Camera::new(world.center(), 26.0).with_pitch(55.0);
        RoutesDemo {
            art,
            world,
            decor,
            camera,
            controller: CameraController::default(),
            towns,
            routes: Vec::new(),
            track: HashSet::new(),
            from: None,
            hover: None,
            preview: None,
            money: 60_000,
            day: 0.0,
            floaters: Vec::new(),
            message: "Click a town, then another town, to survey a railway.".into(),
        }
    }

    fn town_at(&self, h: Hex) -> Option<usize> {
        self.towns.iter().position(|t| t.hex.distance(h) <= 1)
    }

    /// Survey a route: cheap where track exists, dearer uphill, never through water or cliffs.
    fn survey(&self, a: usize, b: usize) -> Option<(Vec<Hex>, u32)> {
        let w = &self.world;
        let track = &self.track;
        let p = path::astar(self.towns[a].hex, self.towns[b].hex, 1, u32::MAX, |x, y| {
            let (tx, ty) = (w.tile(x)?, w.tile(y)?);
            let dh = (ty.height - tx.height).abs();
            if w.is_submerged(y) || dh > 1 {
                return None;
            }
            Some(if track.contains(&edge(x, y)) { 1 } else { 4 + dh as u32 * 8 })
        })?;
        let new_segments = p.hexes.windows(2).filter(|s| !self.track.contains(&edge(s[0], s[1]))).count() as u32;
        Some((p.hexes, new_segments * 600 + p.cost * 50))
    }

    /// 3D polyline along a hex route; level changes get a ramp inside the lower hex.
    fn polyline(&self, hexes: &[Hex]) -> Vec<Vec3> {
        let lift = Vec3::Y * 0.02;
        let mut pts = vec![self.world.hex_to_world(hexes[0]) + lift];
        for s in hexes.windows(2) {
            let (a, b) = (self.world.hex_to_world(s[0]), self.world.hex_to_world(s[1]));
            let mid = (a + b) * 0.5;
            if (a.y - b.y).abs() > 1e-3 {
                let hi = a.y.max(b.y);
                pts.push(Vec3::new(mid.x, hi, mid.z) + lift);
            }
            pts.push(b + lift);
        }
        pts
    }

    fn build(&mut self, a: usize, b: usize, hexes: Vec<Hex>, cost: u32) {
        if self.routes.iter().any(|r| (r.a, r.b) == (a, b) || (r.a, r.b) == (b, a)) {
            self.message = "That line already exists - buy another train for it instead.".into();
            return;
        }
        if self.money < cost as i64 {
            self.message = format!("Not enough money: the line costs ${cost}.");
            return;
        }
        self.money -= cost as i64;
        for s in hexes.windows(2) {
            self.track.insert(edge(s[0], s[1]));
        }
        let points = self.polyline(&hexes);
        let mut lengths = vec![0.0];
        for s in points.windows(2) {
            lengths.push(lengths.last().unwrap() + s[0].distance(s[1]));
        }
        self.message = format!("Opened {} - {} for ${cost}.", self.towns[a].name, self.towns[b].name);
        self.routes.push(Route {
            a,
            b,
            hexes,
            points,
            lengths,
            trains: vec![Train { s: 0.0, dir: 1.0, speed: 3.6, wait: 0.5 }],
            earned: 0,
        });
    }

    /// Rail geometry (textured strips plus ramp embankments).
    fn rail_mesh(&self, assets: &Assets) -> (Vec<Vertex>, Vec<u32>) {
        let region = assets.region(self.art.rail);
        let (u0, v0, u1, v1) = (region.uv0[0], region.uv0[1], region.uv1[0], region.uv1[1]);
        let mut verts = Vec::new();
        let mut idx = Vec::new();
        let mut drawn = HashSet::new();
        for r in &self.routes {
            for s in r.hexes.windows(2) {
                if !drawn.insert(edge(s[0], s[1])) {
                    continue;
                }
                let pts = self.polyline(s);
                for p in pts.windows(2) {
                    let (a, b) = (p[0], p[1]);
                    let d = Vec3::new(b.x - a.x, 0.0, b.z - a.z).normalize_or_zero();
                    let n = Vec3::new(-d.z, 0.0, d.x) * 0.32;
                    let c = [255, 255, 255, 255];
                    let base = verts.len() as u32;
                    verts.extend_from_slice(&[
                        Vertex::new(a + n, [u0, v0], c),
                        Vertex::new(b + n, [u1, v0], c),
                        Vertex::new(b - n, [u1, v1], c),
                        Vertex::new(a - n, [u0, v1], c),
                    ]);
                    idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    // Embankment under ramps.
                    if (a.y - b.y).abs() > 1e-3 {
                        let (lo, hi) = if a.y < b.y { (a, b) } else { (b, a) };
                        let dirt = assets.region(assets.builtin.white).uv(0.5, 0.5);
                        let col = [122, 92, 60, 255];
                        let foot = Vec3::new(hi.x, lo.y, hi.z);
                        let base = verts.len() as u32;
                        verts.extend_from_slice(&[
                            Vertex::new(lo + n * 1.2 - Vec3::Y * 0.01, dirt, col),
                            Vertex::new(hi + n * 1.2 - Vec3::Y * 0.01, dirt, col),
                            Vertex::new(hi - n * 1.2 - Vec3::Y * 0.01, dirt, col),
                            Vertex::new(lo - n * 1.2 - Vec3::Y * 0.01, dirt, col),
                            Vertex::new(foot + n * 1.2, dirt, col),
                            Vertex::new(foot - n * 1.2, dirt, col),
                        ]);
                        idx.extend_from_slice(&[
                            base,
                            base + 1,
                            base + 2,
                            base,
                            base + 2,
                            base + 3,
                            base + 1,
                            base + 4,
                            base + 5,
                            base + 1,
                            base + 5,
                            base + 2,
                        ]);
                    }
                }
            }
        }
        (verts, idx)
    }
}

impl Scene for RoutesDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let dt = ctx.time.dt;
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        self.hover =
            if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)).map(|p| p.hex) };

        let hovered_town = self.hover.and_then(|h| self.town_at(h));
        self.preview = match (self.from, hovered_town) {
            (Some(a), Some(b)) if a != b => self.survey(a, b).map(|(p, c)| (b, p, c)),
            _ => None,
        };
        if ctx.input.clicked(MouseButton::Left) && !over_ui {
            match (self.from, hovered_town) {
                (None, Some(t)) => {
                    self.from = Some(t);
                    self.message = format!("From {}: now click the destination.", self.towns[t].name);
                }
                (Some(a), Some(b)) if a != b => {
                    if let Some((_, p, c)) = self.preview.take() {
                        self.build(a, b, p, c);
                    } else {
                        self.message = "No route possible (water or cliffs).".into();
                    }
                    self.from = None;
                }
                _ => self.from = None,
            }
        }
        if ctx.input.key_pressed(Key::Escape) {
            self.from = None;
        }
        for i in 0..self.routes.len() {
            if ctx.input.ui_clicked(&format!("train{i}")) {
                if self.money >= 5000 {
                    self.money -= 5000;
                    let r = &mut self.routes[i];
                    let s = r.total() * 0.5;
                    r.trains.push(Train { s, dir: -1.0, speed: 3.6, wait: 0.0 });
                } else {
                    self.message = "A train costs $5000.".into();
                }
            }
        }

        // Run trains: accelerate out of stations, earn on arrival, turn around.
        self.day += dt * 2.0;
        let mut income = Vec::new();
        for r in &mut self.routes {
            let total = r.total();
            for t in &mut r.trains {
                if t.wait > 0.0 {
                    t.wait -= dt;
                    continue;
                }
                t.s += t.dir * t.speed * dt;
                if t.s >= total || t.s <= 0.0 {
                    t.s = t.s.clamp(0.0, total);
                    let town = if t.dir > 0.0 { r.b } else { r.a };
                    let pay = 300 + (total * 45.0) as u32;
                    r.earned += pay;
                    income.push((town, pay));
                    t.dir = -t.dir;
                    t.wait = 1.2;
                }
            }
        }
        for (town, pay) in income {
            self.money += pay as i64;
            let p = self.world.hex_to_world(self.towns[town].hex) + Vec3::Y * 2.0;
            ctx.audio.play_at(self.art.sfx.coin, p, &self.camera);
            self.floaters.push(Floater { pos: p, text: format!("+${pay}"), t: 0.0 });
        }
        for f in &mut self.floaters {
            f.t += dt;
            f.pos.y += dt * 0.7;
        }
        self.floaters.retain(|f| f.t < 1.6);
        ctx.status = format!(
            "tycoon towns={} routes={} trains={} money={} track={}",
            self.towns.len(),
            self.routes.len(),
            self.routes.iter().map(|r| r.trains.len()).sum::<usize>(),
            self.money,
            self.track.len()
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let art = self.art.clone();
        let (rail_v, rail_i) = self.rail_mesh(f.assets());
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
            w.raw(&rail_v, &rail_i, true);
            for (i, t) in self.towns.iter().enumerate() {
                let c = w.world.hex_to_world(t.hex);
                for (off, s) in &t.houses {
                    let p = c + Vec3::new(off.x, 0.0, off.y);
                    let p = Vec3::new(p.x, w.world.surface_at(Vec2::new(p.x, p.z)), p.z);
                    w.sprite(Sprite::new(art.village_neutral, p).scale(*s));
                }
                w.sprite(Sprite::new(art.village, c).scale(0.9).bias(0.1));
                let col = if self.from == Some(i) { Color::YELLOW } else { Color::WHITE };
                w.label(c + Vec3::Z * 0.9, t.name, 8.0, col);
                if self.from == Some(i) {
                    w.set_on_top(true);
                    w.ring(c, 1.4, Color::YELLOW);
                    w.set_on_top(false);
                }
            }
            if let Some((_, p, cost)) = &self.preview {
                w.set_on_top(true);
                w.path(p, 0.14, Color::hex(0xffe27a).with_alpha(0.8), false);
                w.set_on_top(false);
                let end = w.world.hex_to_world(*p.last().unwrap()) + Vec3::Y * 2.4;
                let col = if self.money >= *cost as i64 { Color::hex(0x9fffa0) } else { Color::hex(0xff8070) };
                w.label(end, &format!("${cost}  ({} hexes)", p.len()), 16.0, col);
            } else if let Some(h) = self.hover {
                w.hex_outline(h, Color::WHITE.with_alpha(0.5), 0.08);
            }
            for r in &self.routes {
                for t in &r.trains {
                    for (k, img) in [(0.0, art.locomotive), (1.3, art.wagon), (2.4, art.wagon)] {
                        let (p, d) = r.at(t.s - k * t.dir);
                        let facing_left = d.x * t.dir < -0.05;
                        w.shadow(p, 0.5, 0.25);
                        w.sprite(Sprite::new(img, p).scale(0.9).flip(facing_left));
                    }
                }
            }
            for fl in &self.floaters {
                w.label(fl.pos, &fl.text, 16.0, Color::hex(0x9fffa0).with_alpha((1.6 - fl.t).clamp(0.0, 1.0)));
            }
        }
        common::header(
            f,
            "Railroad Tycoon-like",
            "Survey lines with A* over build costs; trains earn money on arrival",
        );
        common::stats(f);
        common::help(
            f,
            &[
                "Click town A, then town B: build a railway (cost shown)",
                "Existing track is reused for free.   Drag: pan   Wheel: zoom",
                &self.message,
            ],
        );

        let size = f.ui_size();
        let t = f.theme.clone();
        let r = Rect::new(size.x - 268.0, 106.0, 260.0, 56.0 + self.routes.len() as f32 * 34.0);
        f.panel(r);
        let year = 1850 + (self.day / 365.0) as i32;
        let month = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
            [((self.day / 30.4) as usize) % 12];
        f.text(
            Vec2::new(r.x + 10.0, r.y + 10.0),
            &format!("${}", self.money),
            16.0,
            if self.money >= 0 { t.accent } else { Color::RED },
        );
        f.text(
            Vec2::new(r.x + 10.0, r.y + 32.0),
            &format!("{month} {year}   {} lines", self.routes.len()),
            8.0,
            t.text_dim,
        );
        for (i, rt) in self.routes.iter().enumerate() {
            let y = r.y + 50.0 + i as f32 * 34.0;
            let name = format!("{}-{}", &self.towns[rt.a].name[..3], &self.towns[rt.b].name[..3]);
            f.text(Vec2::new(r.x + 10.0, y + 4.0), &format!("{name}  {} trains", rt.trains.len()), 8.0, t.text);
            f.text(Vec2::new(r.x + 10.0, y + 16.0), &format!("earned ${}", rt.earned), 8.0, t.text_dim);
            f.button(&format!("train{i}"), Rect::new(r.x + r.w - 96.0, y, 88.0, 26.0), "+Train $5k");
        }
    }
}
