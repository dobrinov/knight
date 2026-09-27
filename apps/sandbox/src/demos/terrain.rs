//! Terrain & camera: procedural maps with elevations and depressions, water, map shapes, hex
//! orientations, viewing angle and render modes.

use std::rc::Rc;

use knight_engine::glam::Vec2;
use knight_engine::hex::{Offset, Parity};
use knight_engine::*;

use crate::art::{Art, Mats};
use crate::common::{self, Decor, MapGen, MapShape};

pub struct TerrainDemo {
    art: Rc<Art>,
    world: HexWorld,
    mats: Mats,
    decor: Vec<Decor>,
    camera: Camera,
    controller: CameraController,
    mapgen: MapGen,
    hover: Option<Pick>,
    show_decor: bool,
}

const PITCHES: [f32; 5] = [30.0, 42.0, 55.0, 70.0, 90.0];

impl TerrainDemo {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let mapgen = MapGen::default();
        let decor = common::generate(&mut world, &mats, &art, &mapgen);
        let camera = Camera::new(world.center(), 30.0);
        TerrainDemo {
            art,
            world,
            mats,
            decor,
            camera,
            controller: CameraController::default(),
            mapgen,
            hover: None,
            show_decor: true,
        }
    }

    fn regenerate(&mut self) {
        self.decor = common::generate(&mut self.world, &self.mats, &self.art, &self.mapgen);
    }
}

impl Scene for TerrainDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let i = &ctx.input;
        let mut regen = false;
        if i.ui_clicked("seed") || i.key_pressed(Key::R) {
            self.mapgen.seed =
                self.mapgen.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 16;
            regen = true;
        }
        if i.ui_clicked("orient") || i.key_pressed(Key::O) {
            self.mapgen.pointy = !self.mapgen.pointy;
            regen = true;
        }
        for (id, shape) in
            [("hexagon", MapShape::Hexagon), ("rect", MapShape::Rectangle), ("para", MapShape::Parallelogram)]
        {
            if i.ui_clicked(id) {
                self.mapgen.shape = shape;
                regen = true;
            }
        }
        if i.ui_clicked("size+") {
            self.mapgen.size = (self.mapgen.size + 8).min(64);
            regen = true;
        }
        if i.ui_clicked("size-") {
            self.mapgen.size = (self.mapgen.size - 8).max(8);
            regen = true;
        }
        if i.ui_clicked("amp+") {
            self.mapgen.amplitude += 3.0;
            regen = true;
        }
        if i.ui_clicked("amp-") {
            self.mapgen.amplitude = (self.mapgen.amplitude - 3.0).max(2.0);
            regen = true;
        }
        if i.ui_clicked("water+") || i.key_pressed(Key::BracketRight) {
            if let Some(w) = &mut self.world.water {
                w.level += 1;
            }
            self.world.mark_all_dirty();
        }
        if i.ui_clicked("water-") || i.key_pressed(Key::BracketLeft) {
            if let Some(w) = &mut self.world.water {
                w.level -= 1;
            }
            self.world.mark_all_dirty();
        }
        if i.ui_clicked("bevel") {
            self.world.bevel = if self.world.bevel > 0.0 { 0.0 } else { 0.12 };
            self.world.mark_all_dirty();
        }
        if i.ui_clicked("bands") {
            self.world.level_bands = !self.world.level_bands;
            self.world.mark_all_dirty();
        }
        if i.ui_clicked("decor") || i.key_pressed(Key::T) {
            self.show_decor = !self.show_decor;
        }
        for (k, p) in PITCHES.iter().enumerate() {
            if i.ui_clicked(&format!("pitch{k}")) {
                self.camera.set_pitch(*p);
            }
        }
        if i.key_pressed(Key::PageUp) {
            self.camera.set_pitch((self.camera.goal_pitch.to_degrees() + 10.0).min(90.0));
        }
        if i.key_pressed(Key::PageDown) {
            self.camera.set_pitch(self.camera.goal_pitch.to_degrees() - 10.0);
        }
        if regen {
            let water = self.world.water.as_ref().map_or(0, |w| w.level);
            self.regenerate();
            if let Some(w) = &mut self.world.water {
                w.level = water;
            }
            self.camera.move_to(self.world.center());
        }

        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, ctx.time.dt, over_ui);
        self.hover = if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)) };
        ctx.status = format!(
            "terrain tiles={} hover={} zoom={:.1} pitch={:.0} cam={:.2},{:.2}",
            self.world.len(),
            self.hover.map_or("-".into(), |p| format!("{},{}", p.hex.q, p.hex.r)),
            self.camera.zoom,
            self.camera.pitch.to_degrees(),
            self.camera.target.x,
            self.camera.target.z
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        {
            let mut w = f.world(&mut self.world, &self.camera);
            if self.show_decor {
                for d in &self.decor {
                    d.draw(&mut w);
                }
            }
            if let Some(p) = self.hover {
                w.hex_fill(p.hex, Color::WHITE.with_alpha(0.18));
                w.hex_outline(p.hex, Color::hex(0xffe27a), 0.12);
            }
        }
        common::header(f, "Terrain & Camera", "Procedural hex terrain with elevations, depressions and water");
        common::stats(f);
        common::help(
            f,
            &[
                "Drag: pan    Wheel / pinch / +-: zoom    WASD: pan",
                "PgUp / PgDn: viewing angle   [ ]: water level",
                "R: new seed   O: pointy / flat   T: trees   P: pixel art",
            ],
        );

        // Options panel (right).
        let size = f.ui_size();
        let r = Rect::new(size.x - 208.0, 106.0, 200.0, 330.0);
        f.panel(r);
        let dim = f.theme.text_dim;
        let mut y = r.y + 10.0;
        let row = |y: &mut f32, h: f32| {
            let out = Rect::new(r.x + 8.0, *y, r.w - 16.0, h);
            *y += h + 6.0;
            out
        };
        f.text(Vec2::new(r.x + 10.0, y), "MAP", 8.0, dim);
        y += 14.0;
        let shapes = row(&mut y, 24.0).cols(3, 4.0);
        f.button_ex("hexagon", shapes[0], "Hex", true, self.mapgen.shape == MapShape::Hexagon);
        f.button_ex("rect", shapes[1], "Rect", true, self.mapgen.shape == MapShape::Rectangle);
        f.button_ex("para", shapes[2], "Rhomb", true, self.mapgen.shape == MapShape::Parallelogram);
        let orient = if self.mapgen.pointy { "Pointy-top hexes [O]" } else { "Flat-top hexes [O]" };
        f.button("orient", row(&mut y, 24.0), orient);
        let c = row(&mut y, 24.0).cols(2, 4.0);
        f.button("size-", c[0], "Smaller");
        f.button("size+", c[1], "Bigger");
        let c = row(&mut y, 24.0).cols(2, 4.0);
        f.button("amp-", c[0], "Flatter");
        f.button("amp+", c[1], "Hillier");
        let c = row(&mut y, 24.0).cols(2, 4.0);
        f.button("water-", c[0], "Water -");
        f.button("water+", c[1], "Water +");
        f.button("seed", row(&mut y, 24.0), "New seed [R]");
        f.text(Vec2::new(r.x + 10.0, y), "VIEW ANGLE", 8.0, dim);
        y += 14.0;
        let c = row(&mut y, 24.0).cols(PITCHES.len(), 3.0);
        for (k, p) in PITCHES.iter().enumerate() {
            let sel = (self.camera.goal_pitch.to_degrees() - p).abs() < 1.0;
            f.button_ex(&format!("pitch{k}"), c[k], &format!("{p:.0}"), true, sel);
        }
        f.text(Vec2::new(r.x + 10.0, y), "LOOK", 8.0, dim);
        y += 14.0;
        let c = row(&mut y, 24.0).cols(3, 4.0);
        f.button_ex("bevel", c[0], "Bevel", true, self.world.bevel > 0.0);
        f.button_ex("bands", c[1], "Bands", true, self.world.level_bands);
        f.button_ex("decor", c[2], "Trees", true, self.show_decor);

        // Hover info.
        if let Some(p) = self.hover {
            let t = self.world.tile(p.hex).unwrap_or_default();
            let mat = &self.world.materials[t.material as usize].name;
            let off = Offset::from_hex_r(p.hex, Parity::Odd);
            let water = if self.world.is_submerged(p.hex) { "  (under water)" } else { "" };
            let text = format!(
                "axial ({}, {})  cube s={}\noffset ({}, {})\nheight {}{}\n{}",
                p.hex.q,
                p.hex.r,
                p.hex.s(),
                off.col,
                off.row,
                t.height,
                water,
                mat
            );
            f.tooltip(&text);
        }
    }
}
