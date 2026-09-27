//! Demo picker, drawn over a slowly drifting generated island.

use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::*;

use crate::art::Art;
use crate::common::{self, Decor, MapGen};
use crate::demos::ENTRIES;

pub struct Menu {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    camera: Camera,
    t: f32,
}

impl Menu {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(&mut world, &mats, &art, &MapGen { seed: 42, size: 30, ..Default::default() });
        let camera = Camera::new(Vec3::ZERO, 40.0).with_pitch(42.0);
        Menu { art, world, decor, camera, t: 0.0 }
    }
}

impl Scene for Menu {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        self.t += ctx.time.dt;
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let a = self.t * 0.05;
        self.camera.target = Vec3::new(a.cos() * 9.0, 0.0, a.sin() * 6.0);
        self.camera.update(ctx.time.dt);
        if ctx.input.key_pressed(Key::P) || ctx.input.ui_clicked("pixel") {
            common::toggle_pixel_mode(ctx);
        }
        common::audio_keys(ctx);
        common::music(ctx, self.art.sfx.music);
        ctx.status = "menu".into();
        let chosen = ENTRIES
            .iter()
            .enumerate()
            .find(|(i, e)| ctx.input.key_pressed(e.key) || ctx.input.ui_clicked(&format!("demo{i}")));
        match chosen {
            Some((_, e)) => Transition::Push((e.open)(self.art.clone())),
            None => Transition::None,
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
        }
        let size = f.ui_size();
        let t = f.theme.clone();
        f.rect(Rect::new(0.0, 0.0, size.x, size.y), Color::BLACK.with_alpha(0.35));
        let title = "KNIGHT ENGINE";
        let m = f.measure(title, 40.0);
        f.text(Vec2::new((size.x - m.x) * 0.5, 40.0), title, 40.0, t.accent);
        let sub = "hex-based isometric 2.5D engine - Rust, wgpu, winit - web, desktop, mobile";
        let ms = f.measure(sub, 8.0);
        f.text(Vec2::new((size.x - ms.x) * 0.5, 40.0 + m.y + 12.0), sub, 8.0, t.text);

        let cols = if size.x > 900.0 {
            3
        } else if size.x > 560.0 {
            2
        } else {
            1
        };
        let card = Vec2::new(((size.x - 48.0) / cols as f32).min(400.0), 70.0);
        let rows = ENTRIES.len().div_ceil(cols);
        let grid =
            Vec2::new(card.x * cols as f32 + 12.0 * (cols - 1) as f32, card.y * rows as f32 + 12.0 * (rows - 1) as f32);
        let origin = Vec2::new((size.x - grid.x) * 0.5, (140.0f32).max((size.y - grid.y) * 0.5));
        // One title size for every card: big unless some title would not fit.
        let big_titles =
            ENTRIES.iter().all(|e| f.measure(&format!("{}  {}", e.label, e.title), 16.0).x <= card.x - 24.0);
        for (i, e) in ENTRIES.iter().enumerate() {
            let (c, r) = (i % cols, i / cols);
            let rect =
                Rect::new(origin.x + c as f32 * (card.x + 12.0), origin.y + r as f32 * (card.y + 12.0), card.x, card.y);
            f.button(&format!("demo{i}"), rect, "");
            let (size, dy) = if big_titles { (16.0, 12.0) } else { (8.0, 16.0) };
            f.text(Vec2::new(rect.x + 12.0, rect.y + dy), &format!("{}  {}", e.label, e.title), size, t.accent);
            f.text_wrapped(Rect::new(rect.x + 12.0, rect.y + 38.0, rect.w - 24.0, 36.0), e.blurb, 8.0, t.text);
        }
        common::stats(f);
        let hint =
            "Press a demo's key or click it.  Esc: back   P: pixel art   M: music   N: mute   Gamepad: stick + A";
        let mh = f.measure(hint, 8.0);
        f.text(Vec2::new((size.x - mh.x) * 0.5, size.y - 28.0), hint, 8.0, t.text_dim);
    }
}
