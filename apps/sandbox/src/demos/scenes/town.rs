//! The town screen: a second world (the castle courtyard) with its own camera plus a build and
//! recruit panel. Buildings appear in the courtyard as they are built.

use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::{Hex, shapes};
use knight_engine::*;

use super::{Purse, Stack, TownResult, add_to_army, stats};
use crate::art::{Art, Team, UnitKind};
use crate::common;

struct Building {
    name: &'static str,
    unit: UnitKind,
    cost: Purse,
    slot: Hex,
}

fn buildings() -> [Building; 5] {
    [
        Building { name: "Barracks", unit: UnitKind::Footman, cost: Purse::new(0, 0, 0), slot: Hex::new(-2, 2) },
        Building { name: "Archery Range", unit: UnitKind::Archer, cost: Purse::new(1000, 5, 0), slot: Hex::new(2, 1) },
        Building { name: "Guardhouse", unit: UnitKind::Ogre, cost: Purse::new(2000, 0, 5), slot: Hex::new(-3, 0) },
        Building { name: "Stables", unit: UnitKind::Knight, cost: Purse::new(3000, 10, 0), slot: Hex::new(3, -2) },
        Building {
            name: "Portal of Glory",
            unit: UnitKind::Griffon,
            cost: Purse::new(6000, 10, 10),
            slot: Hex::new(0, -3),
        },
    ]
}

pub struct TownScene {
    art: Rc<Art>,
    world: HexWorld,
    camera: Camera,
    purse: Purse,
    army: Vec<Stack>,
    built: [bool; 5],
    message: String,
    hovered: Option<usize>,
    t: f32,
}

impl TownScene {
    pub fn new(art: Rc<Art>, purse: Purse, army: Vec<Stack>, built: [bool; 5]) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        world.water = None;
        world.base_height = -3;
        for h in shapes::hexagon(Hex::ORIGIN, 5) {
            let (height, mat) = match h.length() {
                0..=1 => (2, mats.road),
                5 => (0, mats.rock),
                _ => (1, if (h.q + h.r * 3).rem_euclid(5) == 0 { mats.meadow } else { mats.grass }),
            };
            world.set_tile(h, Tile::new(height, mat));
        }
        for b in buildings() {
            world.set_material(b.slot, mats.road);
        }
        let camera = Camera::new(Vec3::new(0.0, 0.4, 0.5), 64.0).with_pitch(35.0);
        TownScene { art, world, camera, purse, army, built, message: String::new(), hovered: None, t: 0.0 }
    }

    fn result(&self) -> TownResult {
        TownResult { purse: self.purse, army: self.army.clone(), built: self.built }
    }
}

impl Scene for TownScene {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        self.t += ctx.time.dt;
        if ctx.input.key_pressed(Key::P) || ctx.input.ui_clicked("pixel") {
            common::toggle_pixel_mode(ctx);
        }
        common::audio_keys(ctx);
        if ctx.input.key_pressed(Key::Escape) || ctx.input.ui_clicked("leave") {
            return Transition::PopWith(Box::new(self.result()));
        }
        // Courtyard camera: fixed framing that adapts to the window (no user control).
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let ui = ctx.screen / ctx.scale_factor;
        let fit = (ui.x * 0.6 / 12.0).min(ui.y / 9.0);
        self.camera.set_zoom(fit, None);
        // Shift the courtyard left of centre, clear of the build panel.
        self.camera.target = Vec3::new(1.6, 0.4, 0.4);
        self.camera.snap();

        for (i, b) in buildings().iter().enumerate() {
            if ctx.input.ui_clicked(&format!("build{i}")) && !self.built[i] {
                if self.purse.covers(b.cost) {
                    self.purse.pay(b.cost);
                    self.built[i] = true;
                    self.message = format!("{} built!", b.name);
                    ctx.audio.play(self.art.sfx.powerup);
                } else {
                    self.message = format!("Not enough resources for the {}.", b.name);
                }
            }
            for n in [1u32, 5] {
                if ctx.input.ui_clicked(&format!("recruit{i}x{n}")) && self.built[i] {
                    let price = stats(b.unit).gold * n;
                    if self.purse.gold < price {
                        self.message = "Not enough gold.".into();
                    } else if add_to_army(&mut self.army, Stack::new(b.unit, n)) {
                        self.purse.gold -= price;
                        ctx.audio.play(self.art.sfx.coin);
                        self.message = format!("Recruited {n} {}.", b.unit.name());
                    } else {
                        self.message = "The army has no free slot.".into();
                    }
                }
            }
        }
        ctx.status = format!(
            "heroes-town gold={} built={} army={}",
            self.purse.gold,
            self.built.iter().filter(|b| **b).count(),
            self.army.iter().map(|s| format!("{}x{}", s.count, s.kind.name())).collect::<Vec<_>>().join("+")
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let size = f.ui_size();
        let art = self.art.clone();
        let mouse = f.ctx.input.mouse;
        let mut hovered = None;
        {
            let mut w = f.world(&mut self.world, &self.camera);
            w.background_gradient(Color::hex(0x24386e), Color::hex(0xa8cce8));
            let keep = w.world.hex_to_world(Hex::ORIGIN);
            w.shadow(keep, 1.6, 0.35);
            w.sprite(Sprite::new(art.keep[0], keep).scale(2.6));
            for (i, b) in buildings().iter().enumerate() {
                let p = w.world.hex_to_world(b.slot);
                if self.built[i] {
                    let img = match i {
                        0 => art.village,
                        1 => art.sawmill,
                        2 => art.keep_neutral,
                        3 => art.village_neutral,
                        _ => art.shrine,
                    };
                    w.shadow(p, 0.9, 0.3);
                    let r = w.sprite(Sprite::new(img, p).scale(1.5));
                    if r.contains(mouse) {
                        hovered = Some(i);
                    }
                    // A recruit standing guard in front.
                    let a = art.unit(b.unit, Team::Blue);
                    let gp = p + Vec3::new(0.7, 0.0, 0.55);
                    w.sprite(Sprite::new(a.idle.frame_at(self.t + i as f32 * 0.3), gp).scale(0.8));
                    w.label(
                        p + Vec3::Y * 0.05 + Vec3::Z * 0.8,
                        b.name,
                        8.0,
                        if hovered == Some(i) { Color::YELLOW } else { Color::WHITE },
                    );
                } else {
                    w.hex_outline(b.slot, Color::WHITE.with_alpha(0.35), 0.06);
                    w.label(p, "empty lot", 8.0, Color::WHITE.with_alpha(0.5));
                }
            }
            w.label(keep + Vec3::Y * 6.2, "KNIGHTSBRIDGE CASTLE", 16.0, Color::hex(0xffe9a8));
        }
        self.hovered = hovered;

        // Build / recruit panel.
        let t = f.theme.clone();
        let r = Rect::new(size.x - 380.0, 106.0, 372.0, 5.0 * 66.0 + 40.0);
        f.panel(r);
        f.text(Vec2::new(r.x + 12.0, r.y + 12.0), "BUILD & RECRUIT", 8.0, t.text_dim);
        for (i, b) in buildings().iter().enumerate() {
            let row = Rect::new(r.x + 8.0, r.y + 30.0 + i as f32 * 66.0, r.w - 16.0, 60.0);
            let highlight = self.hovered == Some(i);
            f.rect(row, if highlight { Color::hex(0x3a3246) } else { Color::hex(0x24202c) });
            let s = stats(b.unit);
            let img = self.art.unit(b.unit, Team::Blue).idle.frame_at(self.t);
            f.image_fit(
                Rect::new(row.x + 2.0, row.y - 2.0, 56.0, 60.0),
                img,
                if self.built[i] { Color::WHITE } else { Color::rgb(0.35, 0.35, 0.4) },
            );
            f.text(Vec2::new(row.x + 62.0, row.y + 6.0), b.name, 8.0, if self.built[i] { t.accent } else { t.text });
            let info = format!(
                "{}  hp {}  dmg {}-{}  spd {}{}",
                b.unit.name(),
                s.hp,
                s.min_dmg,
                s.max_dmg,
                s.speed,
                if s.ranged { "  ranged" } else { "" }
            );
            f.text(Vec2::new(row.x + 62.0, row.y + 20.0), &info, 8.0, t.text_dim);
            if self.built[i] {
                f.text(Vec2::new(row.x + 62.0, row.y + 40.0), &format!("{} gold each", s.gold), 8.0, t.text);
                f.button(&format!("recruit{i}x1"), Rect::new(row.x + row.w - 108.0, row.y + 32.0, 48.0, 22.0), "+1");
                f.button(&format!("recruit{i}x5"), Rect::new(row.x + row.w - 54.0, row.y + 32.0, 48.0, 22.0), "+5");
            } else {
                let c = b.cost;
                let label = format!("Build: {}g {}w {}o", c.gold, c.wood, c.ore);
                f.button_ex(
                    &format!("build{i}"),
                    Rect::new(row.x + 62.0, row.y + 32.0, row.w - 70.0, 22.0),
                    &label,
                    self.purse.covers(c),
                    false,
                );
            }
        }

        // Resources + army + leave.
        let bottom = Rect::new(8.0, size.y - 92.0, size.x - 16.0, 84.0);
        f.panel(bottom);
        let mut x = bottom.x + 12.0;
        for (img, v) in
            [(self.art.gold, self.purse.gold), (self.art.wood, self.purse.wood), (self.art.ore, self.purse.ore)]
        {
            f.image(Rect::new(x, bottom.y + 10.0, 28.0, 28.0), img, Color::WHITE);
            f.text(Vec2::new(x + 32.0, bottom.y + 16.0), &format!("{v}"), 16.0, t.text);
            x += 130.0;
        }
        f.text(Vec2::new(bottom.x + 12.0, bottom.y + 52.0), &self.message, 8.0, t.accent);
        for (i, s) in self.army.iter().enumerate() {
            let cell = Rect::new(bottom.x + 420.0 + i as f32 * 62.0, bottom.y + 6.0, 58.0, 72.0);
            f.rect(cell, Color::hex(0x2a2432));
            f.outline(cell, t.panel_border, 1.0);
            let img = self.art.unit(s.kind, Team::Blue).idle.frames[0];
            f.image_fit(Rect::new(cell.x, cell.y, cell.w, cell.h - 12.0), img, Color::WHITE);
            f.text(Vec2::new(cell.x + 4.0, cell.y + cell.h - 13.0), &format!("{}", s.count), 8.0, Color::WHITE);
        }
        f.button("leave", Rect::new(bottom.x + bottom.w - 150.0, bottom.y + 20.0, 138.0, 44.0), "Leave [Esc]");
        common::stats(f);
    }
}
