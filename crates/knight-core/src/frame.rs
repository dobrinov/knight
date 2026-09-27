//! Per-frame draw recording: world passes (terrain, sprites, overlays) and immediate-mode UI.
//!
//! A [`Frame`] is handed to [`crate::Scene::draw`]. It records renderer-independent geometry;
//! the runtime turns that into GPU work. World geometry goes through a camera; UI is laid out in
//! logical pixels (physical pixels / [`crate::Context::ui_scale`]).

use std::collections::HashSet;

use glam::{Mat4, Vec2, Vec3, Vec4Swizzles};
use knight_hex::Hex;

use crate::assets::{Assets, ImageId};
use crate::mesh::{UiMesh, UiVertex, Vertex, WorldMesh};
use crate::world::{Chunk, HexWorld};
use crate::{Camera, Color, Context, MouseButton, Rect};

/// Stable id for a UI element.
pub fn ui_id(s: &str) -> u64 {
    // FNV-1a
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

/// One camera's worth of world drawing.
#[derive(Clone, Debug, Default)]
pub struct WorldPass {
    pub world_id: u64,
    /// Physical pixels.
    pub viewport: Rect,
    pub view_proj: Mat4,
    /// Fill the viewport first with a vertical gradient (top, bottom): minimap backgrounds,
    /// skies behind a scene.
    pub background: Option<(Color, Color)>,
    /// Terrain chunks in view.
    pub chunks: Vec<Chunk>,
    /// Per-frame geometry: sprites (alpha-tested, depth-written).
    pub opaque: WorldMesh,
    /// Per-frame geometry: overlays, shadows, translucent sprites (blended, drawn after).
    pub transparent: WorldMesh,
    /// Per-frame geometry drawn last without depth testing (routes, selection, markers that
    /// must stay visible behind hills and trees). Not affected by `ambient`.
    pub on_top: WorldMesh,
    /// Light colour multiplied into terrain and sprites (day/night, weather, torchlight).
    pub ambient: Color,
}

/// Everything recorded for one frame. Consumed by the renderer.
#[derive(Default)]
pub struct FrameData {
    pub clear: Color,
    pub passes: Vec<WorldPass>,
    /// UI geometry in physical pixels, drawn last (it includes any scene-transition fade).
    pub ui: UiMesh,
    /// UI rectangles that block pointer input to the world (physical pixels).
    pub ui_rects: Vec<Rect>,
    pub ui_clicked: HashSet<u64>,
}

impl FrameData {
    pub fn reset(&mut self, clear: Color) {
        self.clear = clear;
        self.passes.clear();
        self.ui.clear();
        self.ui_rects.clear();
        self.ui_clicked.clear();
    }
}

/// UI colours.
#[derive(Clone, Debug)]
pub struct Theme {
    pub panel: Color,
    pub panel_border: Color,
    pub panel_highlight: Color,
    pub text: Color,
    pub text_dim: Color,
    pub accent: Color,
    pub button: Color,
    pub button_hover: Color,
    pub button_active: Color,
    pub button_disabled: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            panel: Color::hex(0x1d1a26).with_alpha(0.94),
            panel_border: Color::hex(0x6b5a3e),
            panel_highlight: Color::hex(0xc8a45c),
            text: Color::hex(0xf2ead8),
            text_dim: Color::hex(0xa39a88),
            accent: Color::hex(0xf0c65a),
            button: Color::hex(0x3a3246),
            button_hover: Color::hex(0x51465f),
            button_active: Color::hex(0x7a5c2e),
            button_disabled: Color::hex(0x2a2630),
        }
    }
}

/// Draw text with the built-in 8×8 font at `pos` (physical pixels, top-left). Returns the width.
fn draw_text(ui: &mut UiMesh, assets: &Assets, pos: Vec2, text: &str, glyph: f32, color: Color) -> f32 {
    let mut x = pos.x;
    let mut y = pos.y;
    let mut widest: f32 = 0.0;
    for ch in text.chars() {
        if ch == '\n' {
            widest = widest.max(x - pos.x);
            x = pos.x;
            y += glyph * 1.25;
            continue;
        }
        if ch != ' ' {
            let r = assets.region(assets.glyph(ch));
            ui.rect(Vec2::new(x, y), Vec2::new(x + glyph, y + glyph), &r, color);
        }
        x += glyph;
    }
    widest.max(x - pos.x)
}

/// Snap a text size (logical) to a whole multiple of the 8-pixel font in physical pixels.
fn glyph_px(size: f32, ui_scale: f32) -> f32 {
    ((size * ui_scale / 8.0).round().max(1.0)) * 8.0
}

pub struct Frame<'a> {
    pub ctx: &'a Context,
    pub theme: Theme,
    data: &'a mut FrameData,
    /// UV of the built-in white pixel, for untextured geometry.
    white: [f32; 2],
    click_consumed: bool,
}

impl<'a> Frame<'a> {
    pub fn new(ctx: &'a Context, data: &'a mut FrameData) -> Self {
        let white = ctx.assets.region(ctx.assets.builtin.white).uv(0.5, 0.5);
        Frame { ctx, theme: Theme::default(), data, white, click_consumed: false }
    }

    pub fn assets(&self) -> &Assets {
        &self.ctx.assets
    }

    pub fn clear(&mut self, color: Color) {
        self.data.clear = color;
    }

    /// Darken the whole screen by `fade` (0..1) over everything drawn so far, including the
    /// UI. The scene stack uses it for transitions; call it last if you use it yourself.
    pub fn fade(&mut self, fade: f32) {
        if fade > 0.0 {
            let (white, screen) = (self.white, self.ctx.screen);
            self.data.ui.rect_uv(Vec2::ZERO, screen, white, white, Color::BLACK.with_alpha(fade.min(1.0)));
        }
    }

    // --- World --------------------------------------------------------------------------------

    /// Start drawing a world through a camera. Terrain chunks in view are queued automatically;
    /// use the returned [`WorldDraw`] to add sprites and overlays.
    pub fn world<'b>(&'b mut self, world: &'b mut HexWorld, camera: &Camera) -> WorldDraw<'b> {
        let chunks = world.visible_chunks(camera, &self.ctx.assets);
        let world: &HexWorld = world;
        let view_proj = camera.view_proj();
        self.data.passes.push(WorldPass {
            world_id: world.id,
            viewport: camera.viewport,
            view_proj,
            background: None,
            chunks,
            opaque: WorldMesh::new(),
            transparent: WorldMesh::new(),
            on_top: WorldMesh::new(),
            ambient: Color::WHITE,
        });
        let ui_scale = self.ctx.ui_scale();
        let (right, up, back) = camera.basis();
        WorldDraw {
            pass: self.data.passes.last_mut().unwrap(),
            ui: &mut self.data.ui,
            assets: &self.ctx.assets,
            world,
            camera: camera.clone(),
            view_proj,
            right,
            up,
            back,
            ui_scale,
            time: self.ctx.time.elapsed as f32,
            white: self.white,
            on_top: false,
        }
    }

    // --- UI (logical pixels) ------------------------------------------------------------------

    pub fn ui_scale(&self) -> f32 {
        self.ctx.ui_scale()
    }

    /// Logical screen size.
    pub fn ui_size(&self) -> Vec2 {
        self.ctx.screen / self.ui_scale()
    }

    /// Mouse position in logical pixels.
    pub fn mouse(&self) -> Vec2 {
        self.ctx.input.mouse / self.ui_scale()
    }

    pub fn hovered(&self, r: Rect) -> bool {
        r.contains(self.mouse())
    }

    /// Mark a rectangle as UI so the world ignores pointer input there next frame.
    pub fn block(&mut self, r: Rect) {
        let s = self.ui_scale();
        self.data.ui_rects.push(r.scale(s));
    }

    pub fn rect(&mut self, r: Rect, color: Color) {
        let s = self.ui_scale();
        self.data.ui.rect_uv(r.min() * s, r.max() * s, self.white, self.white, color);
    }

    pub fn outline(&mut self, r: Rect, color: Color, thickness: f32) {
        let t = thickness;
        self.rect(Rect::new(r.x, r.y, r.w, t), color);
        self.rect(Rect::new(r.x, r.y + r.h - t, r.w, t), color);
        self.rect(Rect::new(r.x, r.y + t, t, r.h - 2.0 * t), color);
        self.rect(Rect::new(r.x + r.w - t, r.y + t, t, r.h - 2.0 * t), color);
    }

    pub fn image(&mut self, r: Rect, image: ImageId, tint: Color) {
        let s = self.ui_scale();
        let region = self.ctx.assets.region(image);
        self.data.ui.rect(r.min() * s, r.max() * s, &region, tint);
    }

    /// Draw part of an image: `sub` is a normalised rectangle inside the image (0..1).
    pub fn image_sub(&mut self, r: Rect, image: ImageId, sub: Rect, tint: Color) {
        let s = self.ui_scale();
        let region = self.ctx.assets.region(image);
        let uv0 = region.uv(sub.x, sub.y);
        let uv1 = region.uv(sub.x + sub.w, sub.y + sub.h);
        self.data.ui.rect_uv(r.min() * s, r.max() * s, uv0, uv1, tint);
    }

    /// Draw an image at an integer multiple of its pixel size, centred in `r` (crisp pixel art).
    pub fn image_fit(&mut self, r: Rect, image: ImageId, tint: Color) {
        let region = self.ctx.assets.region(image);
        let s = self.ui_scale();
        let k = ((r.w * s / region.width as f32).min(r.h * s / region.height as f32)).floor().max(1.0);
        let size = Vec2::new(region.width as f32, region.height as f32) * k / s;
        let min = r.center() - size * 0.5;
        let min = (min * s).round() / s;
        self.image(Rect::new(min.x, min.y, size.x, size.y), image, tint);
    }

    /// Width and height of `text` at `size` (logical).
    pub fn measure(&self, text: &str, size: f32) -> Vec2 {
        let g = glyph_px(size, self.ui_scale()) / self.ui_scale();
        let lines = text.split('\n');
        let (mut w, mut n) = (0usize, 0usize);
        for l in lines {
            w = w.max(l.chars().count());
            n += 1;
        }
        Vec2::new(w as f32 * g, g + (n.saturating_sub(1)) as f32 * g * 1.25)
    }

    /// Draw text with its top-left at `pos`. Returns the width. `size` ~ glyph height; it snaps
    /// to whole pixel multiples of the 8×8 font.
    pub fn text(&mut self, pos: Vec2, text: &str, size: f32, color: Color) -> f32 {
        let s = self.ui_scale();
        let g = glyph_px(size, s);
        let p = (pos * s).round();
        let shadow = Color::BLACK.with_alpha(color.a * 0.6);
        let off = (g / 8.0).max(1.0);
        draw_text(&mut self.data.ui, &self.ctx.assets, p + Vec2::splat(off), text, g, shadow);
        draw_text(&mut self.data.ui, &self.ctx.assets, p, text, g, color) / s
    }

    pub fn text_centered(&mut self, r: Rect, text: &str, size: f32, color: Color) {
        let m = self.measure(text, size);
        self.text(r.center() - m * 0.5, text, size, color);
    }

    /// Word-wrapped text inside `r`. Returns the height used.
    pub fn text_wrapped(&mut self, r: Rect, text: &str, size: f32, color: Color) -> f32 {
        let cw = self.measure("M", size).x;
        let max_chars = ((r.w / cw).floor() as usize).max(1);
        let mut lines: Vec<String> = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split_whitespace() {
                if !line.is_empty() && line.len() + 1 + word.len() > max_chars {
                    lines.push(std::mem::take(&mut line));
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
            lines.push(line);
        }
        let lh = self.measure("M", size).y * 1.35;
        for (i, l) in lines.iter().enumerate() {
            self.text(Vec2::new(r.x, r.y + i as f32 * lh), l, size, color);
        }
        lines.len() as f32 * lh
    }

    /// A framed panel that blocks world input.
    pub fn panel(&mut self, r: Rect) {
        self.block(r);
        let t = self.theme.clone();
        self.rect(r, t.panel);
        self.outline(r, t.panel_border, 2.0);
        self.outline(r.inset(2.0), Color::BLACK.with_alpha(0.5), 1.0);
    }

    /// A button. Returns true when clicked this frame. The click is also reported to the next
    /// frame's `ctx.input.ui_clicked(id)` for scenes that handle input in `update`.
    pub fn button(&mut self, id: &str, r: Rect, label: &str) -> bool {
        self.button_ex(id, r, label, true, false)
    }

    pub fn button_ex(&mut self, id: &str, r: Rect, label: &str, enabled: bool, selected: bool) -> bool {
        self.block(r);
        let hovered = enabled && self.hovered(r);
        let t = self.theme.clone();
        let held = hovered && self.ctx.input.down(MouseButton::Left);
        let fill = if !enabled {
            t.button_disabled
        } else if selected || held {
            t.button_active
        } else if hovered {
            t.button_hover
        } else {
            t.button
        };
        self.rect(r, fill);
        self.outline(r, if selected || hovered { t.panel_highlight } else { t.panel_border }, 2.0);
        self.rect(
            Rect::new(r.x + 2.0, r.y + 2.0, r.w - 4.0, 2.0),
            Color::WHITE.with_alpha(if enabled { 0.12 } else { 0.04 }),
        );
        let size = if r.h >= 30.0 && self.measure(label, 16.0).x <= r.w - 12.0 { 16.0 } else { 8.0 };
        self.text_centered(r, label, size, if enabled { t.text } else { t.text_dim });
        let clicked = hovered && !self.click_consumed && self.ctx.input.clicked(MouseButton::Left);
        if clicked {
            self.click_consumed = true;
            self.data.ui_clicked.insert(ui_id(id));
        }
        clicked
    }

    /// A horizontal fill bar (health, progress).
    pub fn bar(&mut self, r: Rect, frac: f32, fill: Color, back: Color) {
        self.rect(r, back);
        self.rect(Rect::new(r.x, r.y, r.w * frac.clamp(0.0, 1.0), r.h), fill);
        self.outline(r, Color::BLACK.with_alpha(0.7), 1.0);
    }

    /// A single-line text field. Click it (or set `focused`) to focus; while focused it takes
    /// typed text, Backspace deletes, Enter submits non-empty text (returns true; the text stays
    /// in `value` for the caller to take) and Escape or clicking elsewhere unfocuses. `focused` is
    /// owned by the caller so it survives frames. A click into the field is reported to next
    /// frame's `ctx.input.ui_clicked(id)` like a button click.
    pub fn text_field(&mut self, id: &str, r: Rect, value: &mut String, focused: &mut bool, placeholder: &str) -> bool {
        self.block(r);
        let input = &self.ctx.input;
        if input.clicked(MouseButton::Left) {
            *focused = self.hovered(r);
            if *focused && !self.click_consumed {
                self.click_consumed = true;
                self.data.ui_clicked.insert(ui_id(id));
            }
        }
        let mut submitted = false;
        if *focused {
            for ch in input.text.chars() {
                if !ch.is_control() && value.chars().count() < 120 {
                    value.push(ch);
                }
            }
            if input.key_pressed(crate::Key::Backspace) {
                value.pop();
            }
            if input.key_pressed(crate::Key::Enter) {
                submitted = !value.trim().is_empty();
            }
            if input.key_pressed(crate::Key::Escape) {
                *focused = false;
            }
        }
        let t = self.theme.clone();
        self.rect(r, Color::hex(0x120f18));
        self.outline(r, if *focused { t.panel_highlight } else { t.panel_border }, 1.0);
        let size = 8.0;
        let cw = self.measure("M", size).x;
        let fit = ((r.w - 12.0) / cw).floor().max(1.0) as usize;
        let shown: String = if value.is_empty() {
            String::new()
        } else {
            let n = value.chars().count();
            value.chars().skip(n.saturating_sub(fit.saturating_sub(1))).collect()
        };
        let y = r.y + (r.h - self.measure("M", size).y) * 0.5;
        if shown.is_empty() && !*focused {
            self.text(Vec2::new(r.x + 6.0, y), placeholder, size, t.text_dim);
        } else {
            let w = self.text(Vec2::new(r.x + 6.0, y), &shown, size, t.text);
            if *focused && (self.ctx.time.elapsed * 2.0) as i64 % 2 == 0 {
                self.rect(Rect::new(r.x + 7.0 + w, y, 2.0, self.measure("M", size).y), t.accent);
            }
        }
        submitted
    }

    /// The on-screen cursor shown while playing with a gamepad.
    pub fn pad_cursor(&mut self) {
        let m = self.mouse();
        let accent = self.theme.accent;
        // A small arrow: dark outline, bright fill.
        for (k, col) in [(2.0, Color::hex(0x1b1420)), (0.0, accent)] {
            for row in 0..12 {
                let w = (row as f32 * 0.6).max(1.0);
                self.rect(Rect::new(m.x - k * 0.5, m.y + row as f32 - k * 0.5, w + k, 1.0 + k * 0.5), col);
            }
        }
    }

    /// A tooltip box next to the mouse.
    pub fn tooltip(&mut self, text: &str) {
        if !self.ctx.input.hovering {
            return;
        }
        let m = self.measure(text, 8.0);
        let size = self.ui_size();
        let mut p = self.mouse() + Vec2::new(14.0, 14.0);
        p.x = p.x.min(size.x - m.x - 12.0);
        p.y = p.y.min(size.y - m.y - 12.0);
        let r = Rect::new(p.x, p.y, m.x + 12.0, m.y + 12.0);
        let t = self.theme.clone();
        self.rect(r, t.panel);
        self.outline(r, t.panel_highlight, 1.0);
        self.text(p + Vec2::splat(6.0), text, 8.0, t.text);
    }
}

/// Draws into one world pass. Created by [`Frame::world`].
pub struct WorldDraw<'b> {
    pass: &'b mut WorldPass,
    ui: &'b mut UiMesh,
    pub assets: &'b Assets,
    pub world: &'b HexWorld,
    pub camera: Camera,
    view_proj: Mat4,
    right: Vec3,
    up: Vec3,
    back: Vec3,
    ui_scale: f32,
    time: f32,
    white: [f32; 2],
    on_top: bool,
}

/// Which per-frame mesh of a [`WorldPass`] a sprite goes to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    /// Alpha-tested, depth-written.
    Opaque,
    /// Blended after the opaque geometry, depth-tested.
    Transparent,
    /// Blended last, unlit, no depth test.
    OnTop,
}

/// An upright, camera-facing image in the world (units, trees, buildings).
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub image: ImageId,
    /// World position of the anchor (usually the feet).
    pub pos: Vec3,
    /// Multiplier on the art size (art is sized by [`HexWorld::pixels_per_unit`]).
    pub scale: f32,
    /// Anchor within the image, `(0.5, 1.0)` = bottom centre.
    pub anchor: Vec2,
    pub tint: Color,
    pub flip_x: bool,
    /// Blend instead of alpha-testing (ghosts, fading units).
    pub translucent: bool,
    /// Move the sprite towards the camera, in world units (to order sprites standing together).
    pub depth_bias: f32,
}

impl Sprite {
    pub fn new(image: ImageId, pos: Vec3) -> Self {
        Sprite {
            image,
            pos,
            scale: 1.0,
            anchor: Vec2::new(0.5, 1.0),
            tint: Color::WHITE,
            flip_x: false,
            translucent: false,
            depth_bias: 0.0,
        }
    }

    pub fn scale(mut self, s: f32) -> Self {
        self.scale = s;
        self
    }

    pub fn tint(mut self, c: Color) -> Self {
        self.tint = c;
        self.translucent |= c.a < 1.0;
        self
    }

    pub fn flip(mut self, f: bool) -> Self {
        self.flip_x = f;
        self
    }

    pub fn anchor(mut self, x: f32, y: f32) -> Self {
        self.anchor = Vec2::new(x, y);
        self
    }

    pub fn bias(mut self, b: f32) -> Self {
        self.depth_bias = b;
        self
    }
}

impl WorldDraw<'_> {
    /// While enabled, overlays (fills, outlines, lines, decals, rings) are drawn on top of
    /// everything instead of being hidden by terrain and sprites in front of them.
    pub fn set_on_top(&mut self, on: bool) {
        self.on_top = on;
    }

    fn overlay_mesh(&mut self) -> &mut WorldMesh {
        if self.on_top { &mut self.pass.on_top } else { &mut self.pass.transparent }
    }

    fn clip_depth(&self, p: Vec3) -> f32 {
        let c = self.view_proj * p.extend(1.0);
        c.z / c.w
    }

    /// Fill the viewport with a colour before drawing (minimaps, split screens).
    pub fn background(&mut self, color: Color) {
        self.pass.background = Some((color, color));
    }

    /// Fill the viewport with a vertical gradient before drawing (skies).
    pub fn background_gradient(&mut self, top: Color, bottom: Color) {
        self.pass.background = Some((top, bottom));
    }

    /// Light colour for this pass: terrain and sprites are multiplied by it (use a dark blue for
    /// night, orange for dusk). The on-top layer stays unlit, so markers and glows stay bright.
    pub fn set_ambient(&mut self, color: Color) {
        self.pass.ambient = color;
    }

    /// Hide terrain (e.g. to draw only sprites through this camera).
    pub fn skip_terrain(&mut self) {
        self.pass.chunks.clear();
    }

    /// Screen position (physical pixels) of a world point.
    pub fn to_screen(&self, p: Vec3) -> Vec2 {
        let c = self.view_proj * p.extend(1.0);
        let n = c.xy() / c.w;
        let v = self.camera.viewport;
        Vec2::new(v.x + (n.x + 1.0) * 0.5 * v.w, v.y + (1.0 - n.y) * 0.5 * v.h)
    }

    /// Draw a sprite. Returns its screen rectangle in physical pixels (for picking/selection).
    pub fn sprite(&mut self, s: Sprite) -> Rect {
        self.sprite_in(s, if s.translucent { Layer::Transparent } else { Layer::Opaque })
    }

    /// A sprite drawn in the unlit on-top layer (glows, sparks, UI-like markers in the world).
    pub fn sprite_on_top(&mut self, s: Sprite) -> Rect {
        self.sprite_in(s, Layer::OnTop)
    }

    fn sprite_in(&mut self, s: Sprite, layer: Layer) -> Rect {
        let region = self.assets.region(s.image);
        let ppu = self.world.pixels_per_unit;
        let w = region.width as f32 / ppu * s.scale;
        let h = region.height as f32 / ppu * s.scale;
        let (l, r) = (-s.anchor.x * w, (1.0 - s.anchor.x) * w);
        let (t, b) = (s.anchor.y * h, -(1.0 - s.anchor.y) * h);
        let (u0, u1) = if s.flip_x { (region.uv1[0], region.uv0[0]) } else { (region.uv0[0], region.uv1[0]) };
        let (v0, v1) = (region.uv0[1], region.uv1[1]);
        let c = s.tint.to_rgba8();
        // Normally a sprite is an upright cut-out standing at its feet: a vertical plane facing
        // the (fixed-direction) camera, stretched by 1/cos(pitch) so it projects at exactly its
        // art size. With real depth it is hidden by higher terrain in front of it and always
        // drawn over terrain behind it, however tall. Near top-down views the plane would
        // degenerate, so there it becomes a billboard sorted at the depth of its feet.
        let cos_pitch = self.camera.pitch.cos();
        let (base, up, depth) = if cos_pitch > 0.26 {
            (s.pos + Vec3::Z * (0.02 + s.depth_bias), Vec3::Y / cos_pitch, -1.0)
        } else {
            (s.pos, self.up, self.clip_depth(s.pos + self.back * (0.35 + s.depth_bias)).max(0.0))
        };
        let at = |x: f32, y: f32| base + self.right * x + up * y;
        let v = |p: Vec3, uv: [f32; 2]| Vertex { pos: p.to_array(), uv, color: c, depth };
        let quad = [v(at(l, t), [u0, v0]), v(at(r, t), [u1, v0]), v(at(r, b), [u1, v1]), v(at(l, b), [u0, v1])];
        match layer {
            Layer::Opaque => self.pass.opaque.quad(quad),
            Layer::Transparent => self.pass.transparent.quad(quad),
            Layer::OnTop => self.pass.on_top.quad(quad),
        }
        let a = self.to_screen(at(l, t));
        let z = self.to_screen(at(r, b));
        Rect::from_corners(a, z)
    }

    /// Flat textured quad lying on the ground (roads, tracks, markers). `angle` rotates it in
    /// the ground plane (radians).
    pub fn decal(&mut self, center: Vec3, image: ImageId, size: Vec2, angle: f32, tint: Color) {
        let region = self.assets.region(image);
        let (s, c) = angle.sin_cos();
        let ax = Vec3::new(c, 0.0, s) * size.x * 0.5;
        let az = Vec3::new(-s, 0.0, c) * size.y * 0.5;
        let p = center + Vec3::Y * 0.015;
        let col = tint.to_rgba8();
        self.overlay_mesh().quad([
            Vertex::new(p - ax - az, region.uv0, col),
            Vertex::new(p + ax - az, [region.uv1[0], region.uv0[1]], col),
            Vertex::new(p + ax + az, region.uv1, col),
            Vertex::new(p - ax + az, [region.uv0[0], region.uv1[1]], col),
        ]);
    }

    /// A soft blob shadow on the ground.
    pub fn shadow(&mut self, pos: Vec3, radius: f32, alpha: f32) {
        let img = self.assets.builtin.circle;
        self.decal(pos, img, Vec2::new(radius * 2.0, radius * 1.4), 0.0, Color::BLACK.with_alpha(alpha));
    }

    /// A ring on the ground (selection).
    pub fn ring(&mut self, pos: Vec3, radius: f32, color: Color) {
        let img = self.assets.builtin.ring;
        self.decal(pos, img, Vec2::splat(radius * 2.0), 0.0, color);
    }

    /// Tint a hex's surface.
    pub fn hex_fill(&mut self, hex: Hex, color: Color) {
        self.hex_fill_inset(hex, color, 0.0);
    }

    /// Tint a hex's surface, shrunk by `inset` (0..1) towards its centre.
    pub fn hex_fill_inset(&mut self, hex: Hex, color: Color, inset: f32) {
        if !self.world.contains(hex) {
            return;
        }
        let c = self.world.hex_to_world(hex) + Vec3::Y * 0.02;
        let white = self.white;
        let col = color.to_rgba8();
        let ring: [Vertex; 6] = std::array::from_fn(|i| {
            let o = self.world.layout.corner_offset(i);
            Vertex::new(c + Vec3::new(o.x, 0.0, o.y) * (1.0 - inset), white, col)
        });
        self.overlay_mesh().fan(Vertex::new(c, white, col), &ring);
    }

    /// Outline a hex's surface. `width` is a fraction of the hex size.
    pub fn hex_outline(&mut self, hex: Hex, color: Color, width: f32) {
        if !self.world.contains(hex) {
            return;
        }
        for edge in 0..6 {
            self.edge_quad(hex, edge, color, width);
        }
    }

    /// A flat ribbon on the ground from `a` to `b`.
    pub fn line(&mut self, a: Vec3, b: Vec3, width: f32, color: Color) {
        let d = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
        let n = Vec3::new(-d.z, 0.0, d.x).normalize_or_zero() * width * 0.5;
        let white = self.white;
        let col = color.to_rgba8();
        let lift = Vec3::Y * 0.03;
        self.overlay_mesh().quad([
            Vertex::new(a + n + lift, white, col),
            Vertex::new(b + n + lift, white, col),
            Vertex::new(b - n + lift, white, col),
            Vertex::new(a - n + lift, white, col),
        ]);
    }

    /// Outline the boundary of a region of hexes (territory borders, selections, zones of
    /// control). `width` is a fraction of the hex size, drawn inside the region.
    pub fn region_outline(&mut self, region: &knight_hex::HexSet, color: Color, width: f32) {
        let dirs: [usize; 6] = std::array::from_fn(|e| self.world.layout.edge_direction(e));
        for &h in region {
            if !self.world.contains(h) {
                continue;
            }
            for (e, &d) in dirs.iter().enumerate() {
                if !region.contains(&h.neighbor(d)) {
                    self.edge_quad(h, e, color, width);
                }
            }
        }
    }

    /// Draw along one edge of a hex (rivers, walls, fences between hexes). `dir` is the
    /// direction of the neighbour across the edge (see [`knight_hex::DIRECTIONS`]).
    pub fn hex_edge(&mut self, hex: Hex, dir: usize, color: Color, width: f32) {
        let edges = self.world.layout.direction_edges();
        self.edge_quad(hex, edges[dir % 6], color, width);
    }

    /// A strip along one edge of `hex`, `width` (fraction of the hex size) wide, inside the hex.
    fn edge_quad(&mut self, hex: Hex, edge: usize, color: Color, width: f32) {
        let c = self.world.hex_to_world(hex) + Vec3::Y * 0.028;
        let white = self.white;
        let col = color.to_rgba8();
        let a = self.world.layout.corner_offset(edge);
        let b = self.world.layout.corner_offset((edge + 1) % 6);
        let (a, b) = (Vec3::new(a.x, 0.0, a.y), Vec3::new(b.x, 0.0, b.y));
        let k = 1.0 - width;
        self.overlay_mesh().quad([
            Vertex::new(c + a, white, col),
            Vertex::new(c + b, white, col),
            Vertex::new(c + b * k, white, col),
            Vertex::new(c + a * k, white, col),
        ]);
    }

    /// A path through hex centres, with an optional dot at each step.
    pub fn path(&mut self, hexes: &[Hex], width: f32, color: Color, dots: bool) {
        let pts: Vec<Vec3> = hexes.iter().map(|&h| self.world.hex_to_world(h)).collect();
        for w in pts.windows(2) {
            self.line(w[0], w[1], width, color);
        }
        if dots {
            for &p in pts.iter().skip(1) {
                self.hex_dot(p, width * 1.8, color);
            }
        }
    }

    fn hex_dot(&mut self, p: Vec3, size: f32, color: Color) {
        let img = self.assets.builtin.circle;
        self.decal(p + Vec3::Y * 0.01, img, Vec2::splat(size), 0.0, color);
    }

    /// Text above a world point (screen-aligned, drawn with the UI so it stays crisp).
    pub fn label(&mut self, pos: Vec3, text: &str, size: f32, color: Color) {
        let g = glyph_px(size, self.ui_scale);
        let s = self.to_screen(pos);
        let w = text.chars().count() as f32 * g;
        let p = (s - Vec2::new(w * 0.5, g)).round();
        let off = (g / 8.0).max(1.0);
        draw_text(self.ui, self.assets, p + Vec2::splat(off), text, g, Color::BLACK.with_alpha(0.7 * color.a));
        draw_text(self.ui, self.assets, p, text, g, color);
    }

    /// A small bar above a world point (health, progress). `width` in logical pixels.
    pub fn bar(&mut self, pos: Vec3, width: f32, frac: f32, color: Color) {
        let s = self.ui_scale;
        let c = self.to_screen(pos);
        let (w, h) = ((width * s).round(), (3.0 * s).round().max(2.0));
        let min = (c - Vec2::new(w * 0.5, h)).round();
        let white = self.white;
        let one = s.round().max(1.0);
        self.ui.rect_uv(
            min - Vec2::splat(one),
            min + Vec2::new(w, h) + Vec2::splat(one),
            white,
            white,
            Color::BLACK.with_alpha(0.8),
        );
        self.ui.rect_uv(min, min + Vec2::new(w * frac.clamp(0.0, 1.0), h), white, white, color);
    }

    /// Seconds since start (for animations).
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Append raw geometry (to the transparent or opaque mesh; the on-top layer if
    /// [`WorldDraw::set_on_top`] is on and `transparent` is set).
    pub fn raw(&mut self, vertices: &[Vertex], indices: &[u32], transparent: bool) {
        let m = if transparent { self.overlay_mesh() } else { &mut self.pass.opaque };
        let base = m.vertices.len() as u32;
        m.vertices.extend_from_slice(vertices);
        m.indices.extend(indices.iter().map(|i| i + base));
    }

    /// UV of the built-in white pixel, for untextured raw geometry.
    pub fn white_uv(&self) -> [f32; 2] {
        self.white
    }

    /// Physical pixels per logical UI pixel.
    pub fn ui_scale(&self) -> f32 {
        self.ui_scale
    }

    /// Glyph size in physical pixels for a logical text size (snapped to the 8×8 font).
    pub fn glyph_size(&self, size: f32) -> f32 {
        glyph_px(size, self.ui_scale)
    }

    /// Screen-space rectangle (physical pixels) drawn with the UI, above the world.
    pub fn ui_rect(&mut self, min: Vec2, max: Vec2, color: Color) {
        self.ui.rect_uv(min, max, self.white, self.white, color);
    }

    /// Screen-space image (physical pixels) drawn with the UI.
    pub fn ui_image(&mut self, min: Vec2, max: Vec2, image: ImageId, tint: Color) {
        let region = self.assets.region(image);
        self.ui.rect(min, max, &region, tint);
    }

    /// Screen-space text at a physical position with a physical glyph size. Returns its width.
    pub fn ui_text(&mut self, pos: Vec2, text: &str, glyph: f32, color: Color) -> f32 {
        draw_text(self.ui, self.assets, pos.round(), text, glyph, color)
    }

    /// Raw UI vertices (physical pixels).
    pub fn raw_ui(&mut self, v: [UiVertex; 4]) {
        self.ui.quad(v);
    }
}
