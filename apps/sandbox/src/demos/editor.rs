//! Picking & editing: elevation-aware picking drives a small map editor (sculpt, paint, add and
//! erase tiles). Only the chunks around an edit are re-meshed. The map can be saved to bytes and
//! loaded back (`HexWorld::to_bytes` / `load_bytes`).

use std::collections::HashSet;
use std::rc::Rc;

use knight_engine::glam::Vec2;
use knight_engine::hex::Hex;
use knight_engine::*;

use crate::art::{Art, Mats};
use crate::common::{self, MapGen};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Raise,
    Lower,
    Flatten,
    Paint,
    Add,
    Erase,
}

const TOOLS: [(Tool, &str, &str); 6] = [
    (Tool::Raise, "Raise", "1"),
    (Tool::Lower, "Lower", "2"),
    (Tool::Flatten, "Flatten", "3"),
    (Tool::Paint, "Paint", "4"),
    (Tool::Add, "Add", "5"),
    (Tool::Erase, "Erase", "6"),
];

pub struct EditorDemo {
    world: HexWorld,
    mats: Mats,
    camera: Camera,
    controller: CameraController,
    tool: Tool,
    brush: i32,
    material: MaterialId,
    hover: Option<Hex>,
    stroke: HashSet<Hex>,
    flatten_to: i16,
    edits: u32,
    /// The last saved map (`HexWorld::to_bytes`); a game would write this to a file.
    saved: Option<Vec<u8>>,
}

impl EditorDemo {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 3, size: 14, amplitude: 6.0, island: 0.6, ..Default::default() },
        );
        let camera = Camera::new(world.center(), 40.0);
        EditorDemo {
            world,
            mats,
            camera,
            controller: CameraController::rts(),
            tool: Tool::Raise,
            brush: 0,
            material: mats.grass,
            hover: None,
            stroke: HashSet::new(),
            flatten_to: 0,
            edits: 0,
            saved: None,
        }
    }

    fn apply(&mut self, center: Hex) {
        for h in center.spiral(self.brush) {
            if !self.stroke.insert(h) {
                continue;
            }
            let t = self.world.tile(h);
            match (self.tool, t) {
                (Tool::Raise, Some(t)) => self.world.set_height(h, (t.height + 1).min(14)),
                (Tool::Lower, Some(t)) => self.world.set_height(h, (t.height - 1).max(-6)),
                (Tool::Flatten, Some(_)) => self.world.set_height(h, self.flatten_to),
                (Tool::Paint, Some(_)) => self.world.set_material(h, self.material),
                (Tool::Add, None) => self.world.set_tile(h, Tile::new(1, self.material)),
                (Tool::Erase, Some(_)) => self.world.remove_tile(h),
                _ => continue,
            }
            self.edits += 1;
        }
    }
}

impl Scene for EditorDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let input = &ctx.input;
        for (i, (tool, _, _)) in TOOLS.iter().enumerate() {
            if input.ui_clicked(&format!("tool{i}")) || input.key_pressed(Key::DIGITS[i + 1]) {
                self.tool = *tool;
            }
        }
        for b in 0..4 {
            if input.ui_clicked(&format!("brush{b}")) {
                self.brush = b;
            }
        }
        if input.key_pressed(Key::B) {
            self.brush = (self.brush + 1) % 4;
        }
        let names: Vec<MaterialId> = (1..self.world.materials.len() as MaterialId).collect();
        for m in names {
            if input.ui_clicked(&format!("mat{m}")) {
                self.material = m;
                if !matches!(self.tool, Tool::Add) {
                    self.tool = Tool::Paint;
                }
            }
        }
        if input.ui_clicked("save") || input.key_pressed(Key::S) && input.ctrl() {
            self.saved = Some(self.world.to_bytes());
        }
        if (input.ui_clicked("load") || input.key_pressed(Key::L) && input.ctrl())
            && let Some(bytes) = &self.saved
        {
            // Materials are matched by name, so the world keeps its textures and costs.
            if let Err(e) = self.world.load_bytes(bytes) {
                log::error!("load: {e}");
            }
            self.edits = 0;
        }

        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, ctx.time.dt, over_ui);

        // Picking: the terrain under the cursor, or the ground plane for adding tiles.
        let ray = self.camera.screen_ray(ctx.input.mouse);
        self.hover = if over_ui {
            None
        } else if self.tool == Tool::Add {
            ray.hit_plane_y(self.world.height_step).map(|p| self.world.world_to_hex(p))
        } else {
            self.world.pick(ray).map(|p| p.hex)
        };

        let input = &ctx.input;
        if input.pressed(MouseButton::Left) {
            self.stroke.clear();
            if let Some(h) = self.hover {
                self.flatten_to = self.world.height(h).unwrap_or(0);
            }
        }
        if input.down(MouseButton::Left)
            && !over_ui
            && let Some(h) = self.hover
        {
            self.apply(h);
        }
        if input.released(MouseButton::Left) {
            self.stroke.clear();
        }
        let _ = self.mats;
        ctx.status = format!(
            "editor tool={:?} brush={} tiles={} edits={} saved={}",
            self.tool,
            self.brush,
            self.world.len(),
            self.edits,
            self.saved.as_ref().map_or(0, |b| b.len())
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        {
            let mut w = f.world(&mut self.world, &self.camera);
            if let Some(h) = self.hover {
                let col = match self.tool {
                    Tool::Erase => Color::RED,
                    Tool::Add => Color::GREEN,
                    _ => Color::hex(0xffe27a),
                };
                for x in h.spiral(self.brush) {
                    w.hex_fill(x, col.with_alpha(0.2));
                    w.hex_outline(x, col, 0.1);
                }
                if self.tool == Tool::Add {
                    // Preview where new tiles would appear (no tile to outline yet).
                    for x in h.spiral(self.brush) {
                        if !w.world.contains(x) {
                            let p = w.world.layout.hex_to_point(x);
                            let c = glam::Vec3::new(p.x, w.world.height_step, p.y);
                            let white = w.assets.builtin.circle;
                            w.decal(c, white, Vec2::splat(1.2), 0.0, Color::GREEN.with_alpha(0.5));
                        }
                    }
                }
            }
        }
        common::header(
            f,
            "Picking & Editing",
            "Rays hit tops and walls of elevated hexes; edits re-mesh only nearby chunks",
        );
        common::stats(f);
        common::help(
            f,
            &[
                "Left click / drag: apply tool     1-6: tools     B: brush size",
                "Right or middle drag: pan         Wheel: zoom    P: pixel art",
                "Ctrl+S / Ctrl+L: save the map to bytes and load it back",
            ],
        );

        let size = f.ui_size();
        let r = Rect::new(size.x - 208.0, 106.0, 200.0, 400.0);
        f.panel(r);
        let dim = f.theme.text_dim;
        f.text(Vec2::new(r.x + 10.0, r.y + 10.0), "TOOL", 8.0, dim);
        let grid = Rect::new(r.x + 8.0, r.y + 24.0, r.w - 16.0, 84.0);
        for (i, row) in grid.rows(3, 6.0).iter().enumerate() {
            for (j, cell) in row.cols(2, 6.0).iter().enumerate() {
                let k = i * 2 + j;
                let (tool, name, key) = TOOLS[k];
                f.button_ex(&format!("tool{k}"), *cell, &format!("{key} {name}"), true, self.tool == tool);
            }
        }
        f.text(Vec2::new(r.x + 10.0, r.y + 118.0), "BRUSH  [B]", 8.0, dim);
        for (b, cell) in Rect::new(r.x + 8.0, r.y + 132.0, r.w - 16.0, 24.0).cols(4, 4.0).iter().enumerate() {
            f.button_ex(&format!("brush{b}"), *cell, &format!("{}", b * 2 + 1), true, self.brush == b as i32);
        }
        f.text(Vec2::new(r.x + 10.0, r.y + 166.0), "MATERIAL", 8.0, dim);
        let mats: Vec<(MaterialId, String, Option<ImageId>)> = self
            .world
            .materials
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, m)| (i as MaterialId, m.name.clone(), m.textures.first().copied()))
            .collect();
        let area = Rect::new(r.x + 8.0, r.y + 180.0, r.w - 16.0, 170.0);
        let rows = mats.len().div_ceil(3);
        let cells: Vec<Rect> = area.rows(rows, 6.0).iter().flat_map(|row| row.cols(3, 6.0)).collect();
        for ((id, name, tex), cell) in mats.iter().zip(cells) {
            f.button_ex(&format!("mat{id}"), cell, "", true, self.material == *id);
            if let Some(t) = tex {
                f.image(cell.inset(4.0), *t, Color::WHITE);
            }
            let short: String = name.chars().take(6).collect();
            f.text(Vec2::new(cell.x + 4.0, cell.y + cell.h - 12.0), &short, 8.0, Color::WHITE);
        }
        f.text(Vec2::new(r.x + 10.0, r.y + 358.0), "MAP", 8.0, dim);
        let c = Rect::new(r.x + 8.0, r.y + 370.0, r.w - 16.0, 24.0).cols(2, 4.0);
        f.button("save", c[0], "Save");
        let label = match &self.saved {
            Some(b) => format!("Load {}b", b.len()),
            None => "Load".to_string(),
        };
        f.button_ex("load", c[1], &label, self.saved.is_some(), false);
        if let Some(h) = self.hover {
            let text = match self.world.tile(h) {
                Some(t) => format!("({}, {})  height {}", h.q, h.r, t.height),
                None => format!("({}, {})  empty", h.q, h.r),
            };
            f.tooltip(&text);
        }
    }
}
