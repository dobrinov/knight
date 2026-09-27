//! Shared sandbox helpers: map generation, animated actors, HUD and render-mode handling.

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::{Hex, Layout, Parity, Rng, ValueNoise, shapes};
use knight_engine::*;

use crate::art::{Art, Mats, Team, UnitArt, UnitKind};

// --- Map generation ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapShape {
    Hexagon,
    Rectangle,
    Parallelogram,
}

#[derive(Clone, Copy, Debug)]
pub struct MapGen {
    pub seed: u64,
    pub shape: MapShape,
    /// Radius for hexagons, half-width for the others.
    pub size: i32,
    pub pointy: bool,
    /// Height levels between sea floor and peaks.
    pub amplitude: f32,
    /// Island falloff (0 = continent, 1 = small island).
    pub island: f32,
    /// Chance of trees on forest hexes.
    pub trees: f32,
}

impl Default for MapGen {
    fn default() -> Self {
        MapGen { seed: 7, shape: MapShape::Hexagon, size: 24, pointy: true, amplitude: 11.0, island: 0.9, trees: 0.7 }
    }
}

/// A static billboard standing on a hex (tree, rock, building).
#[derive(Clone, Copy, Debug)]
pub struct Decor {
    pub hex: Hex,
    pub image: ImageId,
    pub offset: Vec2,
    pub scale: f32,
}

impl Decor {
    pub fn draw(&self, w: &mut WorldDraw) {
        if w.world.visibility(self.hex) == Visibility::Hidden {
            return;
        }
        let p = w.world.hex_to_world(self.hex) + Vec3::new(self.offset.x, 0.0, self.offset.y);
        let tint = if w.world.visibility(self.hex) == Visibility::Explored {
            Color::rgb(0.5, 0.5, 0.55)
        } else {
            Color::WHITE
        };
        w.sprite(Sprite::new(self.image, p).scale(self.scale).tint(tint));
    }
}

pub fn map_hexes(g: &MapGen) -> Vec<Hex> {
    match g.shape {
        MapShape::Hexagon => shapes::hexagon(Hex::ORIGIN, g.size),
        MapShape::Rectangle => {
            let (w, h) = (g.size * 2, (g.size as f32 * 1.6) as i32);
            let off = if g.pointy { Hex::new(-w / 2 + h / 4, -h / 2) } else { Hex::new(-w / 2, -h / 2 + w / 4) };
            shapes::rectangle(w, h, g.pointy, Parity::Odd).into_iter().map(|x| x + off).collect()
        }
        MapShape::Parallelogram => shapes::parallelogram(-g.size, g.size, -g.size * 3 / 4, g.size * 3 / 4),
    }
}

/// Fill `world` with procedural terrain. Returns tree decorations.
pub fn generate(world: &mut HexWorld, mats: &Mats, art: &Art, g: &MapGen) -> Vec<Decor> {
    world.clear();
    world.layout = if g.pointy { Layout::pointy(1.0) } else { Layout::flat(1.0) };
    world.water = Some(Water { level: 0, color: Color::WHITE, texture: Some(art.water) });
    let hexes = map_hexes(g);
    let height = ValueNoise::new(g.seed);
    let wood = ValueNoise::new(g.seed ^ 0xf00d);
    let flowers = ValueNoise::new(g.seed ^ 0xbeef);
    let mut rng = Rng::new(g.seed);
    // Normalise the island falloff by the map's actual extent.
    let pts: Vec<_> = hexes.iter().map(|&h| world.layout.hex_to_point(h)).collect();
    let (cx, cy) = (
        pts.iter().map(|p| p.x).sum::<f32>() / pts.len().max(1) as f32,
        pts.iter().map(|p| p.y).sum::<f32>() / pts.len().max(1) as f32,
    );
    let (ex, ey) =
        pts.iter().fold((1.0f32, 1.0f32), |(ax, ay), p| (ax.max((p.x - cx).abs()), ay.max((p.y - cy).abs())));
    let mut decor = Vec::new();
    for (&h, p) in hexes.iter().zip(&pts) {
        let n = height.fbm(p.x * 0.07, p.y * 0.07, 4);
        let d = (((p.x - cx) / ex).powi(2) + ((p.y - cy) / ey).powi(2)).sqrt();
        let e = n * 1.35 - d.powf(2.2) * g.island + 0.08;
        let level = ((e - 0.35) * g.amplitude).floor().clamp(-4.0, 10.0) as i16;
        let forest = wood.fbm(p.x * 0.12, p.y * 0.12, 2) > 0.55;
        let mat = match level {
            ..0 => mats.seabed,
            0 => mats.sand,
            1..=3 if forest => mats.forest,
            1..=3 if flowers.sample(p.x * 0.2, p.y * 0.2) > 0.7 => mats.meadow,
            1..=3 => mats.grass,
            4..=5 => mats.dirt,
            6..=7 => mats.rock,
            _ => mats.snow,
        };
        world.set_tile(h, Tile::new(level, mat));
        if mat == mats.forest && rng.chance(g.trees) {
            decor.push(Decor {
                hex: h,
                image: art.trees[rng.range(0, 2) as usize],
                offset: Vec2::new(rng.range_f32(-0.2, 0.2), rng.range_f32(-0.15, 0.15)),
                scale: rng.range_f32(0.8, 1.0),
            });
        }
    }
    decor
}

// --- Actors -----------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anim {
    Idle,
    Walk,
    Attack,
    Shoot,
    Hurt,
    Death,
}

/// An animated unit. It moves only through its [`HexMover`]: hex to hex, at a speed set by the
/// terrain it walks onto.
#[derive(Clone, Debug)]
pub struct Actor {
    pub kind: UnitKind,
    pub team: Team,
    /// Render position, derived from the mover on every update.
    pub pos: Vec3,
    pub mover: HexMover,
    /// Steepest step (in height levels) it can walk.
    pub climb: i16,
    /// Fliers ignore terrain and height when moving.
    pub flying: bool,
    pub facing_left: bool,
    pub anim: Anim,
    pub anim_t: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub scale: f32,
    pub selected: bool,
    pub tint: Color,
}

impl Actor {
    pub fn new(kind: UnitKind, team: Team, world: &HexWorld, hex: Hex) -> Self {
        Actor {
            kind,
            team,
            pos: world.hex_to_world(hex),
            mover: HexMover::new(hex, 1.6),
            climb: 1,
            flying: false,
            facing_left: team == Team::Red,
            anim: Anim::Idle,
            anim_t: 0.0,
            hp: 10.0,
            max_hp: 10.0,
            scale: 1.0,
            selected: false,
            tint: Color::WHITE,
        }
    }

    /// The hex it stands on (while stepping, the nearer one).
    pub fn hex(&self) -> Hex {
        self.mover.hex()
    }

    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }

    pub fn moving(&self) -> bool {
        self.mover.is_moving()
    }

    /// Walk a route of adjacent hexes (starting at, or next to, its hex).
    pub fn walk_hexes(&mut self, hexes: &[Hex]) {
        if let Err(e) = self.mover.set_route(hexes) {
            debug_assert!(false, "invalid route: {e}");
            log::warn!("invalid route: {e}");
        }
    }

    /// Put it on a hex instantly (respawn, retreat).
    pub fn place(&mut self, world: &HexWorld, hex: Hex) {
        self.mover.place(hex);
        self.pos = world.hex_to_world(hex);
    }

    pub fn play(&mut self, anim: Anim) {
        if self.anim != anim {
            self.anim = anim;
            self.anim_t = 0.0;
        }
    }

    /// Is a one-shot animation (attack, hurt, death) still running?
    pub fn busy(&self, art: &Art) -> bool {
        match self.anim {
            Anim::Attack | Anim::Shoot | Anim::Hurt => !self.animation(art).finished(self.anim_t),
            _ => false,
        }
    }

    pub fn animation<'a>(&self, art: &'a Art) -> &'a Animation {
        let a: &UnitArt = art.unit(self.kind, self.team);
        match self.anim {
            Anim::Idle => &a.idle,
            Anim::Walk => &a.walk,
            Anim::Attack => &a.attack,
            Anim::Shoot => a.shoot.as_ref().unwrap_or(&a.attack),
            Anim::Hurt => &a.hurt,
            Anim::Death => &a.death,
        }
    }

    /// Advance movement (terrain sets the speed of each step) and animation.
    pub fn update(&mut self, world: &HexWorld, art: &Art, dt: f32) -> Vec<Step> {
        let (climb, flying) = (self.climb, self.flying);
        self.update_with(world, art, dt, |a, b| if flying { Some(1.0) } else { world.step_cost(a, b, climb) })
    }

    /// Like [`Actor::update`] with a custom step rule (reservations, zones of control, ...).
    /// `step(from, to)` returns the time multiplier of a step or `None` to wait.
    pub fn update_with(
        &mut self,
        world: &HexWorld,
        art: &Art,
        dt: f32,
        step: impl FnMut(Hex, Hex) -> Option<f32>,
    ) -> Vec<Step> {
        self.anim_t += dt;
        if !self.alive() {
            self.play(Anim::Death);
            return Vec::new();
        }
        let events = self.mover.update(dt, step);
        if self.mover.blocked_for > 3.0 {
            // A route that stays blocked is abandoned; callers may plan a new one.
            self.mover.stop();
        }
        if self.mover.step().is_some() {
            let h = self.mover.heading(world);
            if h != 0.0 {
                self.facing_left = h < 0.0;
            }
            if !self.busy(art) {
                self.play(Anim::Walk);
            }
        } else if matches!(self.anim, Anim::Walk) || (!self.busy(art) && self.anim != Anim::Idle) {
            self.play(Anim::Idle);
        }
        self.pos = self.mover.position(world);
        events
    }

    /// Draw with a shadow, selection ring and health bar. Returns the screen rect.
    pub fn draw(&self, w: &mut WorldDraw, art: &Art, bar: bool) -> Rect {
        let frame = self.animation(art).frame_at(self.anim_t);
        if self.alive() {
            w.shadow(self.pos, 0.42 * self.scale, 0.35);
        }
        if self.selected {
            w.ring(self.pos, 0.55 * self.scale, Color::hex(0x7cff7c));
        }
        let rect = w.sprite(Sprite::new(frame, self.pos).scale(self.scale).flip(self.facing_left).tint(self.tint));
        if bar && self.alive() && self.hp < self.max_hp {
            let top = self.pos + w.camera.basis().1 * (1.9 * self.scale);
            let col = if self.team == Team::Blue { Color::hex(0x6ee06e) } else { Color::hex(0xff6a5a) };
            w.bar(top, 26.0, self.hp / self.max_hp, col);
        }
        rect
    }
}

// --- Render mode ------------------------------------------------------------------------------

/// Physical pixels per art pixel in pixel-art mode.
pub fn pixel_scale_for(ctx: &Context) -> u32 {
    ctx.scale_factor.round().max(1.0) as u32 * 2
}

/// Configure a camera for the current render mode (pixel snapping and pixel-perfect zoom).
pub fn sync_camera(ctx: &Context, cam: &mut Camera, ppu: f32) {
    cam.viewport = ctx.screen_rect();
    cam.dpr = ctx.scale_factor;
    match ctx.render.mode {
        RenderMode::PixelArt { scale } => {
            // One native art pixel (the bundled art is drawn at 2x) per low-res pixel.
            let unit = scale as f32 * ppu / 2.0 / ctx.scale_factor;
            if cam.zoom_step != Some(unit) {
                cam.pixel_snap = Some(scale as f32);
                cam.zoom_step = Some(unit);
                let z = cam.goal_zoom;
                cam.set_zoom(z, None);
            }
        }
        RenderMode::HiRes { .. } => {
            cam.pixel_snap = None;
            cam.zoom_step = None;
        }
    }
}

pub fn toggle_pixel_mode(ctx: &mut Context) {
    ctx.render.mode = match ctx.render.mode {
        RenderMode::PixelArt { .. } => RenderMode::HiRes { msaa: true },
        RenderMode::HiRes { .. } => RenderMode::PixelArt { scale: pixel_scale_for(ctx) },
    };
}

/// M: music on/off, N: mute everything. Shared by every scene.
pub fn audio_keys(ctx: &mut Context) {
    if ctx.input.key_pressed(Key::N) || ctx.input.ui_clicked("mute") {
        let m = !ctx.audio.muted();
        ctx.audio.set_muted(m);
    }
    if ctx.input.key_pressed(Key::M) || ctx.input.ui_clicked("music") {
        let off = !MUSIC_OFF.with(|c| c.get());
        MUSIC_OFF.with(|c| c.set(off));
        let track = TRACK.with(|t| t.get());
        ctx.audio.play_music(if off { None } else { track });
    }
}

thread_local! {
    static MUSIC_OFF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static TRACK: std::cell::Cell<Option<SoundId>> = const { std::cell::Cell::new(None) };
}

/// Keep `track` playing unless the player switched music off.
pub fn music(ctx: &mut Context, track: SoundId) {
    TRACK.with(|t| t.set(Some(track)));
    let off = MUSIC_OFF.with(|c| c.get());
    ctx.audio.play_music(if off { None } else { Some(track) });
}

/// Weapon sound for a hit at `pos`: a swing or a shot, then the impact.
pub fn strike_sound(ctx: &mut Context, art: &Art, pos: Vec3, ranged: bool, cam: &Camera) {
    ctx.audio.play_at(if ranged { art.sfx.shoot } else { art.sfx.swing }, pos, cam);
    ctx.audio.play_at(art.sfx.hit, pos, cam);
}

/// Play queued positional sounds (for code that has no `Context` at hand).
pub fn flush_sounds(ctx: &mut Context, queue: &mut Vec<(SoundId, Vec3)>, cam: &Camera) {
    for (s, p) in queue.drain(..) {
        ctx.audio.play_at(s, p, cam);
    }
}

/// Keys and buttons every demo shares. Returns true if the demo should exit to the menu.
pub fn common_update(ctx: &mut Context) -> bool {
    if ctx.input.key_pressed(Key::P) || ctx.input.ui_clicked("pixel") {
        toggle_pixel_mode(ctx);
    }
    audio_keys(ctx);
    ctx.input.key_pressed(Key::Escape) || ctx.input.ui_clicked("back")
}

// --- HUD --------------------------------------------------------------------------------------

/// Title bar with a back button (top-left).
pub fn header(f: &mut Frame, title: &str, subtitle: &str) {
    let w = (f.measure(title, 16.0).x.max(f.measure(subtitle, 8.0).x) + 130.0).max(300.0);
    let r = Rect::new(8.0, 8.0, w, 52.0);
    f.panel(r);
    f.button("back", Rect::new(r.x + 8.0, r.y + 10.0, 88.0, 32.0), "< Menu");
    let accent = f.theme.accent;
    let dim = f.theme.text_dim;
    f.text(Vec2::new(r.x + 108.0, r.y + 10.0), title, 16.0, accent);
    f.text(Vec2::new(r.x + 108.0, r.y + 32.0), subtitle, 8.0, dim);
}

/// Help panel listing controls (bottom-left).
pub fn help(f: &mut Frame, lines: &[&str]) {
    let lh = 13.0;
    let w = lines.iter().map(|l| f.measure(l, 8.0).x).fold(0.0, f32::max) + 20.0;
    let h = lines.len() as f32 * lh + 16.0;
    let size = f.ui_size();
    let r = Rect::new(8.0, size.y - h - 8.0, w, h);
    f.panel(r);
    let t = f.theme.text;
    for (i, l) in lines.iter().enumerate() {
        f.text(Vec2::new(r.x + 10.0, r.y + 9.0 + i as f32 * lh), l, 8.0, t);
    }
}

/// Engine stats (top-right) and the pixel-art toggle.
pub fn stats(f: &mut Frame) {
    let ctx = f.ctx;
    let mode = match ctx.render.mode {
        RenderMode::PixelArt { scale } => format!("pixel x{scale}"),
        RenderMode::HiRes { msaa } => format!("hi-res{}", if msaa { " msaa" } else { "" }),
    };
    let lines = [
        format!("{:>5.0} fps  {:>5.1} ms cpu", ctx.time.fps, ctx.stats.cpu_ms),
        format!("{} tris  {} draws", ctx.stats.triangles, ctx.stats.draw_calls),
        format!("chunks {} / {}  {mode}", ctx.stats.chunks_drawn, ctx.stats.chunks_cached),
        format!(
            "{} {}{}",
            short_backend(&ctx.backend),
            if ctx.audio.unlocked { "snd" } else { "snd?" },
            if ctx.input.pad().is_some() { " pad" } else { "" }
        ),
    ];
    let size = f.ui_size();
    let w = 200.0;
    let r = Rect::new(size.x - w - 8.0, 8.0, w, 90.0);
    f.panel(r);
    let dim = f.theme.text_dim;
    for (i, l) in lines.iter().enumerate() {
        f.text(Vec2::new(r.x + 10.0, r.y + 8.0 + i as f32 * 12.0), l, 8.0, dim);
    }
    let row = Rect::new(r.x + 8.0, r.y + 58.0, w - 16.0, 24.0).cols(3, 4.0);
    f.button(
        "pixel",
        Rect::new(row[0].x, row[0].y, row[0].w * 1.6, row[0].h),
        if matches!(ctx.render.mode, RenderMode::PixelArt { .. }) { "Pixel [P]" } else { "Hi-res [P]" },
    );
    let music_on = ctx.audio.music().is_some();
    f.button_ex(
        "music",
        Rect::new(row[1].x + row[1].w * 0.6 + 4.0, row[1].y, row[1].w * 0.7, row[1].h),
        "M",
        true,
        music_on,
    );
    f.button_ex(
        "mute",
        Rect::new(row[2].x + row[2].w * 0.3 + 4.0, row[2].y, row[2].w * 0.7, row[2].h),
        if ctx.audio.muted() { "N x" } else { "N" },
        true,
        !ctx.audio.muted(),
    );
}

fn short_backend(b: &str) -> String {
    let s: String = b.chars().take(24).collect();
    if s.is_empty() { "starting...".into() } else { s }
}
