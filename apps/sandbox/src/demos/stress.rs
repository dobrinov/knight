//! Stress test: maps from 40 thousand to a million hexes (only the chunks in view are meshed,
//! so memory and frame time stay flat however big the map is), a one-image minimap, and
//! thousands of units that navigate strictly hex to hex: each picks a goal, paths
//! to it with A*, reserves every hex before stepping in (no two units ever share one), walks at
//! the speed its terrain allows and re-plans when stuck in traffic.

use std::collections::HashMap;
use std::rc::Rc;

use knight_engine::glam::Vec2;
use knight_engine::hex::path;
use knight_engine::hex::{Hex, Rng};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{self, Actor, Decor, MapGen, MapShape};

/// Map half-widths offered by the size buttons, labelled by hex count (200×160, 512×409 and
/// 1024×819 hexes).
const SIZES: [(i32, &str); 3] = [(100, "32k"), (256, "210k"), (512, "840k")];

pub struct StressDemo {
    art: Rc<Art>,
    world: HexWorld,
    size: i32,
    /// Trees binned by terrain chunk, so only the chunks in view are even looked at.
    decor: HashMap<ChunkKey, Vec<Decor>>,
    minimap: Minimap,
    minimap_rect: Rect,
    gen_ms: f32,
    camera: Camera,
    controller: CameraController,
    units: Vec<Actor>,
    reservations: Reservations,
    land: Vec<Hex>,
    rng: Rng,
    shadows: bool,
    drawn: usize,
    update_ms: f32,
}

impl StressDemo {
    pub fn new(art: Rc<Art>) -> Self {
        Self::with_size(art, SIZES[0].0, true)
    }

    fn with_size(art: Rc<Art>, size: i32, shadows: bool) -> Self {
        let t0 = web_time::Instant::now();
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let trees = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen {
                seed: 2024,
                size,
                shape: MapShape::Rectangle,
                amplitude: 10.0,
                island: 0.45,
                trees: 0.35,
                ..Default::default()
            },
        );
        let mut decor: HashMap<ChunkKey, Vec<Decor>> = HashMap::new();
        for d in trees {
            decor.entry(HexWorld::chunk_of(d.hex)).or_default().push(d);
        }
        // Units start around the middle, so there is a crowd to look at on every map size.
        let centre = world.layout.point_to_hex({
            let c = world.center();
            hex::Point::new(c.x, c.z)
        });
        let mut land: Vec<Hex> = world
            .hexes()
            .filter(|&h| h.distance(centre) <= 70 && world.tile(h).is_some_and(|t| t.height >= 1))
            .collect();
        land.sort();
        let camera = Camera::new(world.center(), 22.0).with_pitch(50.0);
        let gen_ms = t0.elapsed().as_secs_f32() * 1000.0;
        let mut s = StressDemo {
            art,
            world,
            size,
            decor,
            minimap: Minimap::new(256),
            minimap_rect: Rect::default(),
            gen_ms,
            camera,
            controller: CameraController::default(),
            units: Vec::new(),
            reservations: Reservations::new(),
            land,
            rng: Rng::new(1),
            shadows,
            drawn: 0,
            update_ms: 0.0,
        };
        s.add_units(2000);
        s
    }

    fn add_units(&mut self, n: usize) {
        const KINDS: [UnitKind; 5] =
            [UnitKind::Wolf, UnitKind::Footman, UnitKind::Archer, UnitKind::Knight, UnitKind::Ogre];
        let mut placed = 0;
        while placed < n {
            let h = self.land[self.rng.range(0, self.land.len() as i32) as usize];
            let id = self.units.len() as u32 + 1;
            if !self.reservations.reserve(h, id) {
                continue;
            }
            let kind = KINDS[self.rng.range(0, KINDS.len() as i32) as usize];
            let team = if self.rng.chance(0.5) { Team::Blue } else { Team::Red };
            let mut a = Actor::new(kind, team, &self.world, h);
            a.mover.speed = self.rng.range_f32(1.0, 2.2);
            a.scale = 0.8;
            a.anim_t = self.rng.range_f32(0.0, 3.0);
            self.units.push(a);
            placed += 1;
        }
    }
}

impl Scene for StressDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let dt = ctx.time.dt;
        if ctx.input.ui_clicked("more") || ctx.input.key_pressed(Key::Equal) && ctx.input.shift() {
            self.add_units(1000);
        }
        if ctx.input.ui_clicked("less") {
            let n = self.units.len().saturating_sub(1000);
            for i in n..self.units.len() {
                self.reservations.release_all(i as u32 + 1);
            }
            self.units.truncate(n);
        }
        if ctx.input.ui_clicked("shadows") {
            self.shadows = !self.shadows;
        }
        for (size, label) in SIZES {
            if ctx.input.ui_clicked(label) && size != self.size {
                let camera = self.camera.clone();
                *self = Self::with_size(self.art.clone(), size, self.shadows);
                self.camera.zoom = camera.zoom;
                return Transition::None;
            }
        }
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        // Minimap: re-rendered only when the terrain changes; press or drag on it to jump there.
        self.minimap.update(&self.world, &mut ctx.assets, dt);
        let mouse = ctx.input.mouse / ctx.ui_scale();
        let r = self.minimap_rect;
        if (ctx.input.pressed(MouseButton::Left) || ctx.input.down(MouseButton::Left)) && r.contains(mouse) {
            let uv = (mouse - r.min()) / r.size();
            let p = self.minimap.uv_to_world(&self.world, uv);
            self.camera.target.x = p.x;
            self.camera.target.z = p.z;
        }
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);

        let t0 = web_time::Instant::now();
        let (world, res, rng, art) = (&self.world, &mut self.reservations, &mut self.rng, &self.art);
        for (i, a) in self.units.iter_mut().enumerate() {
            let id = i as u32 + 1;
            if !a.moving() {
                // New goal a few hexes away; A* (bounded) around cliffs and water. Hexes held by
                // other units are avoided when planning and waited for when walking.
                let from = a.hex();
                let goal = from + Hex::new(rng.range(-6, 7), rng.range(-6, 7));
                let route = path::astar(from, goal, 1, 60, |x, y| {
                    let c = world.step_points(x, y, 1, 4)?;
                    Some(if res.is_free(y, id) { c } else { c + 12 })
                });
                if let Some(p) = route {
                    a.walk_hexes(&p.hexes);
                }
            }
            let events = a.update_with(world, art, dt, |x, y| {
                let cost = world.step_cost(x, y, 1)?;
                res.reserve(y, id).then_some(cost)
            });
            res.track(id, &events);
            // Stuck in traffic for a moment: give up this route (a new goal is picked next frame).
            if a.mover.blocked_for > 0.6 {
                a.mover.stop();
            }
        }
        self.update_ms = self.update_ms * 0.9 + t0.elapsed().as_secs_f32() * 1000.0 * 0.1;
        let (meshed, verts) = self.world.mesh_stats();
        ctx.status = format!(
            "stress tiles={} units={} reserved={} drawn={} chunks={} meshed={} mesh_mb={:.1} fps={:.0} gen_ms={:.0} cam={:.0},{:.0} zoom={:.1}",
            self.world.len(),
            self.units.len(),
            self.reservations.len(),
            self.drawn,
            ctx.stats.chunks_drawn,
            meshed,
            mesh_mb(verts),
            ctx.time.fps,
            self.gen_ms,
            self.camera.target.x,
            self.camera.target.z,
            self.camera.zoom,
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        {
            let keys = self.world.visible_chunk_keys(&self.camera);
            let mut w = f.world(&mut self.world, &self.camera);
            let view = w.camera.viewport.inset(-80.0);
            for d in keys.iter().filter_map(|k| self.decor.get(k)).flatten() {
                let p = w.world.hex_to_world(d.hex);
                if view.contains(w.to_screen(p)) {
                    d.draw(&mut w);
                }
            }
            let mut drawn = 0;
            for a in &self.units {
                // Cheap culling: skip units whose feet are well off screen.
                if !view.contains(w.to_screen(a.pos)) {
                    continue;
                }
                drawn += 1;
                if self.shadows {
                    w.shadow(a.pos, 0.35, 0.3);
                }
                let frame = a.animation(&self.art).frame_at(a.anim_t);
                w.sprite(Sprite::new(frame, a.pos).scale(a.scale).flip(a.facing_left));
            }
            self.drawn = drawn;
        }
        common::header(f, "Stress Test", "Huge maps: only visible chunks are meshed + minimap + batched sprites");
        common::stats(f);
        common::help(
            f,
            &[
                "Drag or screen edge: pan   Wheel: zoom   Minimap: click to jump",
                "Units path hex to hex, reserve hexes (never overlap), walk at terrain speed",
                "P: pixel art   Esc: menu",
            ],
        );
        let size = f.ui_size();
        let r = Rect::new(size.x - 208.0, 106.0, 200.0, 220.0);
        f.panel(r);
        let t = f.theme.clone();
        f.text(Vec2::new(r.x + 10.0, r.y + 10.0), &format!("{} hexes", self.world.len()), 8.0, t.text);
        let (meshed, verts) = self.world.mesh_stats();
        f.text(
            Vec2::new(r.x + 10.0, r.y + 122.0),
            &format!("mesh {meshed} chunks {:.1} MB", mesh_mb(verts)),
            8.0,
            t.text_dim,
        );
        f.text(Vec2::new(r.x + 10.0, r.y + 136.0), &format!("map built in {:.0} ms", self.gen_ms), 8.0, t.text_dim);
        f.text(Vec2::new(r.x + 10.0, r.y + 158.0), "Map size", 8.0, t.text);
        let c = Rect::new(r.x + 8.0, r.y + 176.0, r.w - 16.0, 26.0).cols(3, 4.0);
        for (i, (s, label)) in SIZES.iter().enumerate() {
            f.button_ex(label, c[i], label, true, *s == self.size);
        }
        // Minimap in the bottom-right corner, with the camera's view outlined.
        if let Some(img) = self.minimap.image() {
            let w = 220.0f32.min(size.x * 0.35);
            let h = (w / self.minimap.aspect()).min(size.y * 0.35);
            let w = h * self.minimap.aspect();
            let m = Rect::new(size.x - w - 12.0, size.y - h - 12.0, w, h);
            f.panel(m.inset(-4.0));
            f.image(m, img, Color::WHITE);
            if let Some((lo, hi)) = self.minimap.view_rect(&self.world, &self.camera) {
                let v = Rect::new(m.x + lo.x * m.w, m.y + lo.y * m.h, (hi.x - lo.x) * m.w, (hi.y - lo.y) * m.h);
                f.outline(v, Color::WHITE, 1.0);
            }
            f.block(m.inset(-4.0));
            self.minimap_rect = m;
        }
        f.text(
            Vec2::new(r.x + 10.0, r.y + 24.0),
            &format!("{} units ({} drawn)", self.units.len(), self.drawn),
            8.0,
            t.text,
        );
        f.text(Vec2::new(r.x + 10.0, r.y + 38.0), &format!("unit update {:.2} ms", self.update_ms), 8.0, t.text_dim);
        let c = Rect::new(r.x + 8.0, r.y + 56.0, r.w - 16.0, 26.0).cols(2, 4.0);
        f.button("less", c[0], "-1000");
        f.button("more", c[1], "+1000");
        f.button_ex("shadows", Rect::new(r.x + 8.0, r.y + 88.0, r.w - 16.0, 26.0), "Shadows", true, self.shadows);
    }
}

fn mesh_mb(verts: usize) -> f32 {
    (verts * std::mem::size_of::<mesh::Vertex>()) as f32 / 1_048_576.0
}
