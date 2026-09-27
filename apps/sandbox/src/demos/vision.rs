//! Vision & fog: height-aware field of view (hills and forests block sight, high ground sees
//! further), line-of-sight probes and fog of war with explored memory.

use std::collections::HashSet;
use std::rc::Rc;

use knight_engine::glam::Vec2;
use knight_engine::hex::path;
use knight_engine::hex::vision::{field_of_view, line_of_sight};
use knight_engine::hex::{Hex, HexSet};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{self, Actor, Decor, MapGen};

pub struct VisionDemo {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    forest: HashSet<Hex>,
    camera: Camera,
    controller: CameraController,
    scout: Actor,
    radius: i32,
    eye: f32,
    fov: HexSet,
    fov_from: Option<Hex>,
    hover: Option<Hex>,
    fog: bool,
}

impl VisionDemo {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 21, size: 20, amplitude: 12.0, island: 0.6, trees: 0.8, ..Default::default() },
        );
        let forest = decor.iter().map(|d| d.hex).collect();
        world.set_fog_enabled(true);
        let start = world.hexes().filter(|&h| !world.is_submerged(h)).min_by_key(|h| h.length()).unwrap_or(Hex::ORIGIN);
        let mut scout = Actor::new(UnitKind::Archer, Team::Blue, &world, start);
        scout.mover.speed = 2.0;
        let camera = Camera::new(world.hex_to_world(start), 34.0);
        VisionDemo {
            art,
            world,
            decor,
            forest,
            camera,
            controller: CameraController::rts(),
            scout,
            radius: 7,
            eye: 1.5,
            fov: HexSet::default(),
            fov_from: None,
            hover: None,
            fog: true,
        }
    }

    /// Blocking height in levels: terrain, plus 2 for trees.
    fn blocker(&self) -> impl Fn(Hex) -> f32 + '_ {
        move |h| match self.world.tile(h) {
            Some(t) => {
                t.height.max(self.world.water.as_ref().map_or(i16::MIN, |w| w.level)) as f32
                    + if self.forest.contains(&h) { 2.0 } else { 0.0 }
            }
            None => -100.0,
        }
    }

    fn refresh_fov(&mut self, force: bool) {
        let at = self.scout.hex();
        if !force && self.fov_from == Some(at) {
            return;
        }
        self.fov_from = Some(at);
        // Stand on the ground (trees at your own hex don't block you).
        let world = &self.world;
        let forest = &self.forest;
        let origin_height = world.height(at).unwrap_or(0) as f32;
        let height = |h: Hex| {
            if h == at {
                return origin_height;
            }
            match world.tile(h) {
                Some(t) => t.height.max(0) as f32 + if forest.contains(&h) { 2.0 } else { 0.0 },
                None => -100.0,
            }
        };
        self.fov = field_of_view(at, self.radius, self.eye, height);
        let visible: Vec<Hex> = self.fov.iter().copied().filter(|h| self.world.contains(*h)).collect();
        self.world.update_fog(visible);
    }
}

impl Scene for VisionDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let input = &ctx.input;
        let mut force = false;
        if input.key_pressed(Key::F) || input.ui_clicked("fog") {
            self.fog = !self.fog;
            self.world.set_fog_enabled(self.fog);
            force = true;
        }
        if input.ui_clicked("r+") {
            self.radius = (self.radius + 1).min(14);
            force = true;
        }
        if input.ui_clicked("r-") {
            self.radius = (self.radius - 1).max(2);
            force = true;
        }
        if input.ui_clicked("eye+") {
            self.eye += 1.0;
            force = true;
        }
        if input.ui_clicked("eye-") {
            self.eye = (self.eye - 1.0).max(0.5);
            force = true;
        }
        if input.ui_clicked("forget") {
            self.world.set_fog_enabled(false);
            self.world.set_fog_enabled(self.fog);
            force = true;
        }
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, ctx.time.dt, over_ui);
        self.hover =
            if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)).map(|p| p.hex) };
        if ctx.input.clicked(MouseButton::Left)
            && !over_ui
            && let Some(goal) = self.hover
        {
            let w = &self.world;
            let start = self.scout.hex();
            let route = path::astar(start, goal, 1, u32::MAX, |a, b| {
                let (ta, tb) = (w.tile(a)?, w.tile(b)?);
                (!w.is_submerged(b) && (tb.height - ta.height).abs() <= 1).then_some(1)
            });
            if let Some(p) = route {
                self.scout.walk_hexes(&p.hexes);
            }
        }
        self.scout.update(&self.world, &self.art, ctx.time.dt);
        self.refresh_fov(force);
        let explored = self.world.hexes().filter(|&h| self.world.visibility(h) != Visibility::Hidden).count();
        ctx.status = format!("vision visible={} explored={} fog={}", self.fov.len(), explored, self.fog);
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let at = self.scout.hex();
        let los = self.hover.map(|h| {
            let b = self.blocker();
            let origin = self.world.height(at).unwrap_or(0) as f32;
            let height = |x: Hex| if x == at { origin } else { b(x) };
            (h, line_of_sight(at, h, self.eye, 0.05, true, height))
        });
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
            if !self.fog {
                for &h in &self.fov {
                    w.hex_fill(h, Color::hex(0xfff1a0).with_alpha(0.16));
                }
            }
            if let Some((h, ok)) = los {
                let col = if ok { Color::hex(0x6ef08a) } else { Color::hex(0xff5a4a) };
                let line = at.line_to(h);
                for &x in &line[1..] {
                    w.hex_outline(x, col.with_alpha(0.6), 0.08);
                }
                let a = w.world.hex_to_world(at) + glam::Vec3::Y * self.eye * w.world.height_step;
                let b = w.world.hex_to_world(h);
                w.line(a, b, 0.06, col);
                w.hex_outline(h, col, 0.14);
                w.label(b, if ok { "visible" } else { "blocked" }, 8.0, col);
            }
            self.scout.draw(&mut w, &self.art, false);
        }
        common::header(
            f,
            "Vision & Fog",
            "Field of view over heights: hills and forests block sight, high ground sees far",
        );
        common::stats(f);
        common::help(
            f,
            &[
                "Click: move the scout (paths avoid cliffs)   Hover: line-of-sight probe",
                "F: fog on/off   Right drag: pan   Wheel: zoom   P: pixel art",
                "Fog: black = never seen, dim = explored, clear = visible now",
            ],
        );
        let size = f.ui_size();
        let r = Rect::new(size.x - 208.0, 106.0, 200.0, 170.0);
        f.panel(r);
        let dim = f.theme.text_dim;
        f.button_ex("fog", Rect::new(r.x + 8.0, r.y + 8.0, r.w - 16.0, 24.0), "Fog of war [F]", true, self.fog);
        f.text(Vec2::new(r.x + 10.0, r.y + 42.0), &format!("SIGHT RADIUS  {}", self.radius), 8.0, dim);
        let c = Rect::new(r.x + 8.0, r.y + 54.0, r.w - 16.0, 22.0).cols(2, 4.0);
        f.button("r-", c[0], "-");
        f.button("r+", c[1], "+");
        f.text(Vec2::new(r.x + 10.0, r.y + 86.0), &format!("EYE HEIGHT  {:.1} levels", self.eye), 8.0, dim);
        let c = Rect::new(r.x + 8.0, r.y + 98.0, r.w - 16.0, 22.0).cols(2, 4.0);
        f.button("eye-", c[0], "-");
        f.button("eye+", c[1], "+");
        f.button("forget", Rect::new(r.x + 8.0, r.y + 132.0, r.w - 16.0, 24.0), "Forget explored");
    }
}
