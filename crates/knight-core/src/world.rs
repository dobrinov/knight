//! Hex terrain: tiles with heights and materials, water, fog of war, meshing and picking.
//!
//! Terrain is split into chunks of `CHUNK × CHUNK` axial hexes. Chunks are meshed **lazily**,
//! only when a camera sees them (nearest first, a few per frame), and meshes not seen for a
//! while are evicted — so memory and load time depend on what is on screen, not on map size.
//! Each chunk keeps a cached mesh
//! that is rebuilt only when a tile (or a neighbour, or its fog) changes, so large maps cost
//! nothing to redraw while the camera moves, rotates or tilts.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use glam::{Vec2, Vec3};
use knight_hex::{Hex, HexBuildHasher, HexMap, HexSet, Layout, Point};

use crate::Color;
use crate::assets::{Assets, ImageId};
use crate::geom::Ray;
use crate::mesh::{Vertex, WorldMesh};

pub const CHUNK: i32 = 16;

/// Chunk-keyed maps use the same cheap hasher as [`HexMap`].
type ChunkMap<T> = HashMap<ChunkKey, T, HexBuildHasher>;
type ChunkSet = HashSet<ChunkKey, HexBuildHasher>;

pub type MaterialId = u16;

/// How a material looks.
#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    /// Tint for the top face (multiplied with the texture, if any).
    pub top: Color,
    /// Tint for the side walls.
    pub side: Color,
    /// Top textures; one variant is picked per hex from its coordinates.
    pub textures: Vec<ImageId>,
    /// Wall texture. Without [`Material::side_texture_size`] one copy is stretched over each
    /// height level.
    pub side_texture: Option<ImageId>,
    /// Size of the wall texture in world units (width along the wall, height). When set, walls
    /// tile it at its true size (texel-accurate pixel-art cliffs) instead of once per level.
    pub side_texture_size: Option<Vec2>,
    /// How slow it is to walk onto: 1 = normal, 0.5 = road (twice as fast), 2 = forest (half
    /// speed). Used by [`HexWorld::step_cost`] for real-time speed and turn-based movement points.
    pub move_cost: f32,
    /// Can walkers enter it at all (lava, chasms)?
    pub walkable: bool,
}

impl Material {
    pub fn color(name: &str, top: Color, side: Color) -> Self {
        Material {
            name: name.to_string(),
            top,
            side,
            textures: Vec::new(),
            side_texture: None,
            side_texture_size: None,
            move_cost: 1.0,
            walkable: true,
        }
    }

    pub fn with_move_cost(mut self, cost: f32) -> Self {
        self.move_cost = cost;
        self
    }

    pub fn textured(name: &str, textures: Vec<ImageId>, side: Color) -> Self {
        Material {
            name: name.to_string(),
            top: Color::WHITE,
            side,
            textures,
            side_texture: None,
            side_texture_size: None,
            move_cost: 1.0,
            walkable: true,
        }
    }
}

/// One hex column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tile {
    /// Height in levels (see [`HexWorld::height_step`]). Negative values are depressions.
    pub height: i16,
    pub material: MaterialId,
}

impl Tile {
    pub const fn new(height: i16, material: MaterialId) -> Self {
        Tile { height, material }
    }
}

/// A water plane: every tile below `level` is drawn submerged.
#[derive(Clone, Debug)]
pub struct Water {
    pub level: i16,
    pub color: Color,
    pub texture: Option<ImageId>,
}

/// Fog-of-war state of one hex.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Visibility {
    #[default]
    Hidden,
    Explored,
    Visible,
}

/// Result of picking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    pub hex: Hex,
    /// World point where the ray hit.
    pub point: Vec3,
}

/// Chunk coordinates (see [`HexWorld::chunk_of`]).
pub type ChunkKey = (i32, i32);

/// Cached geometry of one chunk. Meshes are shared with the renderer through `Arc`.
#[derive(Clone, Debug)]
pub struct Chunk {
    pub key: ChunkKey,
    pub version: u64,
    pub opaque: Arc<WorldMesh>,
    pub transparent: Arc<WorldMesh>,
    pub min: Vec3,
    pub max: Vec3,
    /// Frame (see [`HexWorld::visible_chunks`]) this chunk was last drawn in.
    pub last_used: u64,
}

static NEXT_WORLD_ID: AtomicU64 = AtomicU64::new(1);

/// A hex terrain world.
pub struct HexWorld {
    /// Unique id (the renderer keys its GPU buffers on it).
    pub id: u64,
    pub layout: Layout,
    /// World units per height level.
    pub height_step: f32,
    /// Walls at the map edge go down to this level.
    pub base_height: i16,
    pub materials: Vec<Material>,
    pub water: Option<Water>,
    /// Darkening of hex corners relative to the centre (0 = flat shading). Makes the grid
    /// readable without drawing lines.
    pub bevel: f32,
    /// Darken each height level's wall band alternately so levels can be counted.
    pub level_bands: bool,
    /// Direction light comes from (normalised). Lights walls; tops are unlit.
    pub light: Vec3,
    /// Art pixels per world unit, used to size sprites (`64 px / 32 = 2 world units`).
    pub pixels_per_unit: f32,
    /// Most chunks meshed per [`HexWorld::visible_chunks`] call (spreads the cost of big jumps
    /// or zooming out over a few frames).
    pub mesh_budget: usize,
    /// Most meshed chunks kept in memory; least recently seen ones are dropped beyond this.
    pub mesh_cache: usize,
    tiles: HexMap<Tile>,
    fog: Option<HexMap<Visibility>>,
    /// The hexes currently [`Visibility::Visible`] (so fog updates cost only what changed).
    visible: HexSet,
    chunks: ChunkMap<Chunk>,
    dirty: ChunkSet,
    /// Tiles per chunk (which chunks exist at all).
    occupancy: ChunkMap<u32>,
    frame: u64,
    version: u64,
    fog_version: u64,
    min_height: i16,
    max_height: i16,
}

impl HexWorld {
    pub fn new(layout: Layout) -> Self {
        HexWorld {
            id: NEXT_WORLD_ID.fetch_add(1, Ordering::Relaxed),
            layout,
            height_step: 0.35,
            base_height: -2,
            materials: vec![Material::color("default", Color::hex(0x6fa35a), Color::hex(0x7a5a3c))],
            water: None,
            bevel: 0.12,
            level_bands: true,
            light: Vec3::new(-0.6, 0.7, 0.4).normalize(),
            pixels_per_unit: 32.0,
            tiles: HexMap::new(),
            fog: None,
            visible: HexSet::default(),
            mesh_budget: 48,
            mesh_cache: 1024,
            chunks: ChunkMap::default(),
            dirty: ChunkSet::default(),
            occupancy: ChunkMap::default(),
            frame: 0,
            fog_version: 0,
            version: 1,
            min_height: 0,
            max_height: 0,
        }
    }

    pub fn add_material(&mut self, m: Material) -> MaterialId {
        self.materials.push(m);
        self.mark_all_dirty();
        (self.materials.len() - 1) as MaterialId
    }

    pub fn material_by_name(&self, name: &str) -> Option<MaterialId> {
        self.materials.iter().position(|m| m.name == name).map(|i| i as MaterialId)
    }

    // --- Tiles --------------------------------------------------------------------------------

    pub fn tile(&self, h: Hex) -> Option<Tile> {
        self.tiles.get(h).copied()
    }

    pub fn contains(&self, h: Hex) -> bool {
        self.tiles.contains(h)
    }

    pub fn tiles(&self) -> impl Iterator<Item = (Hex, Tile)> + '_ {
        self.tiles.iter().map(|(h, t)| (h, *t))
    }

    pub fn hexes(&self) -> impl Iterator<Item = Hex> + '_ {
        self.tiles.hexes()
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn set_tile(&mut self, h: Hex, t: Tile) {
        if self.tiles.get(h) == Some(&t) {
            return;
        }
        if self.tiles.insert(h, t).is_none() {
            *self.occupancy.entry(chunk_of(h)).or_default() += 1;
        }
        self.min_height = self.min_height.min(t.height);
        self.max_height = self.max_height.max(t.height);
        self.touch(h);
    }

    pub fn remove_tile(&mut self, h: Hex) {
        if self.tiles.remove(h).is_some() {
            let k = chunk_of(h);
            if let Some(n) = self.occupancy.get_mut(&k) {
                *n -= 1;
                if *n == 0 {
                    self.occupancy.remove(&k);
                }
            }
            self.touch(h);
        }
    }

    pub fn set_height(&mut self, h: Hex, height: i16) {
        if let Some(t) = self.tile(h) {
            self.set_tile(h, Tile { height, ..t });
        }
    }

    pub fn set_material(&mut self, h: Hex, material: MaterialId) {
        if let Some(t) = self.tile(h) {
            self.set_tile(h, Tile { material, ..t });
        }
    }

    /// Remove every tile (fog of war, if on, starts over hidden).
    pub fn clear(&mut self) {
        self.tiles.clear();
        self.chunks.clear();
        self.dirty.clear();
        self.occupancy.clear();
        if let Some(fog) = &mut self.fog {
            fog.clear();
            self.visible.clear();
            self.fog_version += 1;
        }
        self.min_height = 0;
        self.max_height = 0;
        self.version += 1;
    }

    /// Height level of a hex (`None` off the map).
    pub fn height(&self, h: Hex) -> Option<i16> {
        self.tiles.get(h).map(|t| t.height)
    }

    /// Is this tile under water?
    pub fn is_submerged(&self, h: Hex) -> bool {
        match (&self.water, self.tiles.get(h)) {
            (Some(w), Some(t)) => t.height < w.level,
            _ => false,
        }
    }

    /// World-space y of a hex's ground (top face).
    pub fn ground_y(&self, h: Hex) -> f32 {
        self.height(h).unwrap_or(self.base_height) as f32 * self.height_step
    }

    /// World-space y of what you'd stand on: the ground, or the water surface above it.
    pub fn surface_y(&self, h: Hex) -> f32 {
        let g = self.ground_y(h);
        match &self.water {
            Some(w) if self.is_submerged(h) => w.level as f32 * self.height_step,
            _ => g,
        }
    }

    /// Centre of a hex's surface in world space.
    pub fn hex_to_world(&self, h: Hex) -> Vec3 {
        let p = self.layout.hex_to_point(h);
        Vec3::new(p.x, self.surface_y(h), p.y)
    }

    /// Ground position (x, z) → hex.
    pub fn world_to_hex(&self, p: Vec3) -> Hex {
        self.layout.point_to_hex(Point::new(p.x, p.z))
    }

    /// Surface height at an arbitrary world x/z (the height of the hex it falls in).
    pub fn surface_at(&self, p: Vec2) -> f32 {
        self.surface_y(self.layout.point_to_hex(Point::new(p.x, p.y)))
    }

    /// Movement cost of walking onto `h` (its material's [`Material::move_cost`]).
    pub fn move_cost(&self, h: Hex) -> Option<f32> {
        let t = self.tiles.get(h)?;
        let m = &self.materials[(t.material as usize).min(self.materials.len() - 1)];
        m.walkable.then_some(m.move_cost)
    }

    /// Cost of one step for a walker that can climb `max_climb` levels: `None` when the step is
    /// impossible (off the map, under water, unwalkable, a cliff), otherwise the destination's
    /// move cost plus 50% per level climbed. Real-time movers use it as a time multiplier;
    /// turn-based games as movement points (see [`HexWorld::step_points`]).
    pub fn step_cost(&self, from: Hex, to: Hex, max_climb: i16) -> Option<f32> {
        let (a, b) = (self.tiles.get(from)?, self.tiles.get(to)?);
        if self.is_submerged(to) {
            return None;
        }
        let dh = b.height - a.height;
        if dh.abs() > max_climb {
            return None;
        }
        Some(self.move_cost(to)? * (1.0 + 0.5 * dh.max(0) as f32))
    }

    /// [`HexWorld::step_cost`] in whole movement points, `scale` points for a normal step (use
    /// e.g. 10 so roads cost 5 and forests 20).
    pub fn step_points(&self, from: Hex, to: Hex, max_climb: i16, scale: u32) -> Option<u32> {
        self.step_cost(from, to, max_climb).map(|c| ((c * scale as f32).round() as u32).max(1))
    }

    /// Ground-plane bounds (x/z) of all tiles.
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for h in self.tiles.hexes() {
            let p = self.layout.hex_to_point(h);
            lo = lo.min(Vec2::new(p.x, p.y));
            hi = hi.max(Vec2::new(p.x, p.y));
        }
        if lo.x > hi.x { (Vec2::ZERO, Vec2::ZERO) } else { (lo, hi) }
    }

    pub fn center(&self) -> Vec3 {
        let (lo, hi) = self.bounds();
        let c = (lo + hi) * 0.5;
        Vec3::new(c.x, 0.0, c.y)
    }

    pub fn height_range(&self) -> (i16, i16) {
        (self.min_height, self.max_height)
    }

    // --- Fog of war ---------------------------------------------------------------------------

    /// Turn fog of war on (every hex starts hidden) or off.
    pub fn set_fog_enabled(&mut self, on: bool) {
        if on == self.fog.is_some() {
            return;
        }
        self.fog = on.then(HexMap::new);
        self.visible.clear();
        self.fog_version += 1;
        self.mark_all_dirty();
    }

    pub fn fog_enabled(&self) -> bool {
        self.fog.is_some()
    }

    pub fn visibility(&self, h: Hex) -> Visibility {
        match &self.fog {
            None => Visibility::Visible,
            Some(f) => f.get(h).copied().unwrap_or_default(),
        }
    }

    pub fn is_visible(&self, h: Hex) -> bool {
        self.visibility(h) == Visibility::Visible
    }

    pub fn set_visibility(&mut self, h: Hex, v: Visibility) {
        let Some(fog) = &mut self.fog else { return };
        let old = fog.get(h).copied().unwrap_or_default();
        if old == v {
            return;
        }
        fog.insert(h, v);
        if v == Visibility::Visible {
            self.visible.insert(h);
        } else {
            self.visible.remove(&h);
        }
        self.fog_version += 1;
        let k = chunk_of(h);
        if self.chunks.contains_key(&k) {
            self.dirty.insert(k);
        }
    }

    /// The hexes currently visible (empty without fog of war).
    pub fn visible_hexes(&self) -> impl Iterator<Item = Hex> + '_ {
        self.visible.iter().copied()
    }

    /// Replace the visible set: previously visible hexes become explored, `visible` become
    /// visible. Costs only the hexes that change, and only chunks whose fog actually changed
    /// are rebuilt, so it is fine to call every frame.
    pub fn update_fog(&mut self, visible: impl IntoIterator<Item = Hex>) {
        if self.fog.is_none() {
            return;
        }
        let now: HexSet = visible.into_iter().collect();
        let gone: Vec<Hex> = self.visible.iter().filter(|h| !now.contains(h)).copied().collect();
        for h in gone {
            self.set_visibility(h, Visibility::Explored);
        }
        for h in now {
            self.set_visibility(h, Visibility::Visible);
        }
    }

    // --- Picking ------------------------------------------------------------------------------

    /// First tile surface (top or wall) hit by a ray, e.g. from [`crate::Camera::screen_ray`].
    pub fn pick(&self, ray: Ray) -> Option<Pick> {
        if self.tiles.is_empty() || ray.dir.y >= 0.0 {
            return None;
        }
        let water = self.water.as_ref().map(|w| w.level).unwrap_or(i16::MIN);
        let top = self.max_height.max(water) as f32 * self.height_step + 0.01;
        let bottom = self.min_height.min(self.base_height) as f32 * self.height_step - 0.01;
        let t0 = (top - ray.origin.y) / ray.dir.y;
        let t1 = (bottom - ray.origin.y) / ray.dir.y;
        let horizontal = Vec2::new(ray.dir.x, ray.dir.z).length();
        let step = if horizontal < 1e-4 { (t1 - t0).max(0.0) } else { 0.04 * self.layout.size.x / horizontal };
        let mut t = t0.max(0.0);
        let mut n = 0;
        while t <= t1 + step && n < 20_000 {
            let p = ray.at(t);
            let h = self.world_to_hex(p);
            if self.tiles.contains(h) && p.y <= self.surface_y(h) + 1e-4 {
                // Refine: binary search between the previous sample and this one.
                let (mut a, mut b) = ((t - step).max(t0), t);
                for _ in 0..12 {
                    let m = (a + b) * 0.5;
                    let pm = ray.at(m);
                    let hm = self.world_to_hex(pm);
                    if self.tiles.contains(hm) && pm.y <= self.surface_y(hm) + 1e-4 {
                        b = m;
                    } else {
                        a = m;
                    }
                }
                let p = ray.at(b);
                return Some(Pick { hex: self.world_to_hex(p), point: p });
            }
            t += step;
            n += 1;
        }
        None
    }

    // --- Meshing ------------------------------------------------------------------------------

    fn touch(&mut self, h: Hex) {
        self.version += 1;
        // Only meshes that exist can go stale; unbuilt chunks are meshed fresh when seen (so
        // bulk map generation before the first frame costs nothing here).
        if self.chunks.is_empty() {
            return;
        }
        for k in std::iter::once(chunk_of(h)).chain(h.neighbors().into_iter().map(chunk_of)) {
            if self.chunks.contains_key(&k) {
                self.dirty.insert(k);
            }
        }
    }

    /// Throw away all meshes (they are rebuilt as they come into view). Use after changing
    /// materials, water, lighting or fog settings.
    pub fn mark_all_dirty(&mut self) {
        self.version += 1;
        self.chunks.clear();
        self.dirty.clear();
    }

    /// Bumped on every tile change; handy for invalidating game-side caches.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Bumped whenever any hex's fog state changes.
    pub fn fog_version(&self) -> u64 {
        self.fog_version
    }

    /// Which chunk a hex belongs to.
    pub fn chunk_of(h: Hex) -> ChunkKey {
        chunk_of(h)
    }

    /// Mesh every chunk now (small maps, tests, precomputing). Big maps don't need this:
    /// [`HexWorld::visible_chunks`] meshes on demand.
    pub fn prepare(&mut self, assets: &Assets) {
        let mut keys: Vec<ChunkKey> = self.occupancy.keys().copied().filter(|k| !self.chunks.contains_key(k)).collect();
        keys.extend(self.dirty.drain());
        for key in keys {
            self.rebuild(key, assets);
        }
    }

    fn rebuild(&mut self, key: ChunkKey, assets: &Assets) {
        self.dirty.remove(&key);
        match self.build_chunk(key, assets) {
            Some(mut c) => {
                c.last_used = self.frame;
                self.chunks.insert(key, c);
            }
            None => {
                self.chunks.remove(&key);
            }
        }
    }

    pub fn chunks(&self) -> impl Iterator<Item = &Chunk> {
        self.chunks.values()
    }

    /// Lowest and highest world y any surface can have.
    fn y_range(&self) -> (f32, f32) {
        let water = self.water.as_ref().map(|w| w.level);
        let lo = self.min_height.min(self.base_height).min(water.unwrap_or(i16::MAX));
        let hi = self.max_height.max(water.unwrap_or(i16::MIN));
        (lo as f32 * self.height_step, hi as f32 * self.height_step + 0.05)
    }

    /// Conservative world-space box of a chunk, without meshing it.
    fn chunk_bounds(&self, key: ChunkKey) -> (Vec3, Vec3) {
        if let Some(c) = self.chunks.get(&key) {
            return (c.min, c.max);
        }
        let (q0, r0) = (key.0 * CHUNK, key.1 * CHUNK);
        let ext = self.layout.hex_extent();
        let (y0, y1) = self.y_range();
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for (q, r) in [(q0, r0), (q0 + CHUNK - 1, r0), (q0, r0 + CHUNK - 1), (q0 + CHUNK - 1, r0 + CHUNK - 1)] {
            let p = self.layout.hex_to_point(Hex::new(q, r));
            lo = lo.min(Vec2::new(p.x, p.y));
            hi = hi.max(Vec2::new(p.x, p.y));
        }
        (Vec3::new(lo.x - ext.x, y0, lo.y - ext.y), Vec3::new(hi.x + ext.x, y1, hi.y + ext.y))
    }

    /// Chunks (that contain tiles) inside a camera's view, nearest to the view centre first.
    /// Cost is proportional to what is on screen, not to the map size. Use it to cull your own
    /// objects too (bin them by [`HexWorld::chunk_of`]).
    pub fn visible_chunk_keys(&self, camera: &crate::Camera) -> Vec<ChunkKey> {
        let (y0, y1) = self.y_range();
        let v = camera.viewport;
        let corners = [
            Vec2::new(v.x, v.y),
            Vec2::new(v.x + v.w, v.y),
            Vec2::new(v.x, v.y + v.h),
            Vec2::new(v.x + v.w, v.y + v.h),
        ];
        // The ground footprint of the view between the lowest and highest terrain.
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for c in corners {
            let ray = camera.screen_ray(c);
            for y in [y0, y1] {
                if let Some(p) = ray.hit_plane_y(y) {
                    lo = lo.min(Vec2::new(p.x, p.z));
                    hi = hi.max(Vec2::new(p.x, p.z));
                }
            }
        }
        if lo.x > hi.x {
            return Vec::new();
        }
        let pad = self.layout.hex_extent().y;
        let (lo, hi) = (lo - Vec2::splat(pad), hi + Vec2::splat(pad));
        let (mut qmin, mut qmax, mut rmin, mut rmax) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for (x, y) in [(lo.x, lo.y), (hi.x, lo.y), (lo.x, hi.y), (hi.x, hi.y)] {
            let f = self.layout.point_to_frac(Point::new(x, y));
            qmin = qmin.min(f.q);
            qmax = qmax.max(f.q);
            rmin = rmin.min(f.r);
            rmax = rmax.max(f.r);
        }
        let ck = |v: f64| (v.floor() as i32).div_euclid(CHUNK);
        let target = Vec2::new(camera.target.x, camera.target.z);
        let view_proj = camera.view_proj();
        let mut keys: Vec<(f32, ChunkKey)> = Vec::new();
        // Walk the smaller of the view range and the set of existing chunks.
        let span = ((ck(qmax) - ck(qmin) + 1) as i64) * ((ck(rmax) - ck(rmin) + 1) as i64);
        let mut consider = |key: ChunkKey| {
            let (a, b) = self.chunk_bounds(key);
            if crate::Camera::box_in_view(&view_proj, a, b) {
                let c = Vec2::new((a.x + b.x) * 0.5, (a.z + b.z) * 0.5);
                keys.push((c.distance_squared(target), key));
            }
        };
        if span as usize > self.occupancy.len() {
            for &key in self.occupancy.keys() {
                if (ck(qmin)..=ck(qmax)).contains(&key.0) && (ck(rmin)..=ck(rmax)).contains(&key.1) {
                    consider(key);
                }
            }
        } else {
            for cq in ck(qmin)..=ck(qmax) {
                for cr in ck(rmin)..=ck(rmax) {
                    if self.occupancy.contains_key(&(cq, cr)) {
                        consider((cq, cr));
                    }
                }
            }
        }
        keys.sort_by(|a, b| a.0.total_cmp(&b.0));
        keys.into_iter().map(|(_, k)| k).collect()
    }

    /// Chunks a camera sees, meshed and ready to draw. Meshes missing or stale chunks (nearest
    /// first, at most [`HexWorld::mesh_budget`] per call) and evicts meshes that haven't been
    /// seen for a while beyond [`HexWorld::mesh_cache`].
    pub fn visible_chunks(&mut self, camera: &crate::Camera, assets: &Assets) -> Vec<Chunk> {
        self.frame += 1;
        let keys = self.visible_chunk_keys(camera);
        let mut built = 0;
        let mut out = Vec::with_capacity(keys.len());
        for key in &keys {
            let stale = !self.chunks.contains_key(key) || self.dirty.contains(key);
            if stale && built < self.mesh_budget {
                built += 1;
                self.rebuild(*key, assets);
            }
            if let Some(c) = self.chunks.get_mut(key) {
                c.last_used = self.frame;
                out.push(c.clone());
            }
        }
        // Stale meshes out of view are simply dropped; they are rebuilt when seen again.
        let visible: HashSet<ChunkKey> = keys.into_iter().collect();
        let gone: Vec<ChunkKey> = self.dirty.iter().copied().filter(|k| !visible.contains(k)).collect();
        for k in gone {
            self.dirty.remove(&k);
            self.chunks.remove(&k);
        }
        let cap = self.mesh_cache.max(visible.len() * 2);
        if self.chunks.len() > cap {
            let mut by_age: Vec<(u64, ChunkKey)> = self.chunks.values().map(|c| (c.last_used, c.key)).collect();
            by_age.sort_unstable();
            for (_, k) in by_age.into_iter().take(self.chunks.len() - cap) {
                self.chunks.remove(&k);
            }
        }
        out
    }

    /// (meshed chunks, total vertices) currently held in memory.
    pub fn mesh_stats(&self) -> (usize, usize) {
        let verts = self.chunks.values().map(|c| c.opaque.vertices.len() + c.transparent.vertices.len()).sum();
        (self.chunks.len(), verts)
    }

    fn build_chunk(&self, key: ChunkKey, assets: &Assets) -> Option<Chunk> {
        let mut opaque = WorldMesh::new();
        let mut transparent = WorldMesh::new();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let edge_dirs: [usize; 6] = std::array::from_fn(|e| self.layout.edge_direction(e));
        let offsets: [Point; 6] = std::array::from_fn(|i| self.layout.corner_offset(i));
        let faces_viewer: [bool; 6] = std::array::from_fn(|e| offsets[e].y + offsets[(e + 1) % 6].y > 1e-4);
        let white = assets.region(assets.builtin.white);
        let white_uv = white.uv(0.5, 0.5);
        let extent = self.layout.hex_extent();
        let step = self.height_step;
        let mut any = false;
        for dq in 0..CHUNK {
            for dr in 0..CHUNK {
                let h = Hex::new(key.0 * CHUNK + dq, key.1 * CHUNK + dr);
                let Some(tile) = self.tiles.get(h).copied() else { continue };
                any = true;
                let mat = &self.materials[(tile.material as usize).min(self.materials.len() - 1)];
                let fog = match self.visibility(h) {
                    Visibility::Visible => 1.0,
                    Visibility::Explored => 0.5,
                    Visibility::Hidden => 0.12,
                };
                let c = self.layout.hex_to_point(h);
                let y = tile.height as f32 * step;
                let corner = |i: usize, y: f32| Vec3::new(c.x + offsets[i].x, y, c.y + offsets[i].y);
                lo = lo.min(Vec3::new(c.x - extent.x, self.base_height.min(tile.height) as f32 * step, c.y - extent.y));
                hi = hi.max(Vec3::new(
                    c.x + extent.x,
                    y.max(self.water.as_ref().map_or(y, |w| w.level as f32 * step)),
                    c.y + extent.y,
                ));

                // Top face.
                let top_region = if mat.textures.is_empty() {
                    None
                } else {
                    let v = (h.q.wrapping_mul(7919) ^ h.r.wrapping_mul(104_729)).unsigned_abs() as usize;
                    Some(assets.region(mat.textures[v % mat.textures.len()]))
                };
                let uv_at = |off: Point| match &top_region {
                    Some(r) => r.uv(off.x / extent.x + 0.5, off.y / extent.y + 0.5),
                    None => white_uv,
                };
                let top_col = |k: f32| mat.top.shade(k * fog).to_rgba8();
                let hub = Vertex::new(Vec3::new(c.x, y, c.y), uv_at(Point::new(0.0, 0.0)), top_col(1.0));
                let ring: [Vertex; 6] =
                    std::array::from_fn(|i| Vertex::new(corner(i, y), uv_at(offsets[i]), top_col(1.0 - self.bevel)));
                opaque.fan(hub, &ring);

                // Walls: each edge facing a lower neighbour (or the map edge). The camera always
                // looks north, so walls whose outward normal points away from it (−z) are never
                // visible and are skipped.
                for (e, &dir) in edge_dirs.iter().enumerate() {
                    if !faces_viewer[e] {
                        continue;
                    }
                    let n = h.neighbor(dir);
                    let floor = self.tiles.get(n).map_or(self.base_height, |t| t.height);
                    if floor >= tile.height {
                        continue;
                    }
                    let a = offsets[e];
                    let b = offsets[(e + 1) % 6];
                    let normal = Vec3::new(a.x + b.x, 0.0, a.y + b.y).normalize_or_zero();
                    let lit = 0.55
                        + 0.45 * normal.dot(Vec3::new(self.light.x, 0.0, self.light.z).normalize_or_zero()).max(0.0);
                    let side_region = mat.side_texture.map(|t| assets.region(t));
                    if let (Some(r), Some(size)) = (&side_region, mat.side_texture_size) {
                        // Texel-accurate: tile the texture at its world size, measured down from
                        // the column top (so a grass lip stays on top). Quads are split where
                        // one copy of the texture ends, since the atlas can't wrap.
                        let col = mat.side.shade(lit * fog).to_rgba8();
                        let len = Vec2::new(b.x - a.x, b.y - a.y).length();
                        let u_span = (len / size.x).min(1.0);
                        let u0 = (e as f32 * len / size.x).fract().min(1.0 - u_span);
                        let (bottom, top) = (floor as f32 * step, tile.height as f32 * step);
                        let mut y = top;
                        while y > bottom + 1e-4 {
                            let depth = top - y;
                            let copy_top = (depth / size.y + 1e-4).floor() * size.y;
                            let next = (top - copy_top - size.y).max(bottom);
                            let (v0, v1) = ((depth - copy_top) / size.y, (top - next - copy_top) / size.y);
                            opaque.quad([
                                Vertex::new(corner(e, y), r.uv(u0, v0), col),
                                Vertex::new(corner((e + 1) % 6, y), r.uv(u0 + u_span, v0), col),
                                Vertex::new(corner((e + 1) % 6, next), r.uv(u0 + u_span, v1), col),
                                Vertex::new(corner(e, next), r.uv(u0, v1), col),
                            ]);
                            y = next;
                        }
                        continue;
                    }
                    for level in floor..tile.height {
                        let band = if self.level_bands && level.rem_euclid(2) == 1 { 0.9 } else { 1.0 };
                        let col = mat.side.shade(lit * band * fog).to_rgba8();
                        let (y0, y1) = ((level + 1) as f32 * step, level as f32 * step);
                        let (uv0, uv1) = match &side_region {
                            Some(r) => (r.uv0, r.uv1),
                            None => (white_uv, white_uv),
                        };
                        opaque.quad([
                            Vertex::new(corner(e, y0), [uv0[0], uv0[1]], col),
                            Vertex::new(corner((e + 1) % 6, y0), [uv1[0], uv0[1]], col),
                            Vertex::new(corner((e + 1) % 6, y1), [uv1[0], uv1[1]], col),
                            Vertex::new(corner(e, y1), [uv0[0], uv1[1]], col),
                        ]);
                    }
                }

                // Water surface (and its outer walls at the map edge).
                if let Some(w) = &self.water
                    && tile.height < w.level
                {
                    let wy = w.level as f32 * step;
                    let wr = w.texture.map(|t| assets.region(t));
                    let wuv = |off: Point| match &wr {
                        Some(r) => r.uv(off.x / extent.x + 0.5, off.y / extent.y + 0.5),
                        None => white_uv,
                    };
                    let depth = (w.level - tile.height) as f32;
                    let alpha = (0.55 + 0.1 * depth).min(0.85);
                    let col = w.color.shade(fog).with_alpha(alpha).to_rgba8();
                    let edge = w.color.shade(fog * 1.15).with_alpha(alpha).to_rgba8();
                    let hub = Vertex::new(Vec3::new(c.x, wy, c.y), wuv(Point::new(0.0, 0.0)), col);
                    let ring: [Vertex; 6] = std::array::from_fn(|i| Vertex::new(corner(i, wy), wuv(offsets[i]), edge));
                    transparent.fan(hub, &ring);
                    for (e, &dir) in edge_dirs.iter().enumerate() {
                        if !faces_viewer[e] || self.tiles.contains(h.neighbor(dir)) {
                            continue;
                        }
                        let col = w.color.shade(0.8 * fog).with_alpha(alpha).to_rgba8();
                        let by = self.base_height as f32 * step;
                        transparent.quad([
                            Vertex::new(corner(e, wy), white_uv, col),
                            Vertex::new(corner((e + 1) % 6, wy), white_uv, col),
                            Vertex::new(corner((e + 1) % 6, by), white_uv, col),
                            Vertex::new(corner(e, by), white_uv, col),
                        ]);
                    }
                }
            }
        }
        any.then(|| Chunk {
            key,
            last_used: 0,
            version: self.version,
            opaque: Arc::new(opaque),
            transparent: Arc::new(transparent),
            min: lo,
            max: hi,
        })
    }
}

fn chunk_of(h: Hex) -> ChunkKey {
    (h.q.div_euclid(CHUNK), h.r.div_euclid(CHUNK))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Camera;
    use knight_hex::shapes;

    fn world() -> HexWorld {
        let mut w = HexWorld::new(Layout::pointy(1.0));
        for h in shapes::hexagon(Hex::ORIGIN, 6) {
            w.set_tile(h, Tile::new(0, 0));
        }
        w
    }

    #[test]
    fn meshing_builds_chunks_and_walls() {
        let assets = Assets::new();
        let mut w = world();
        w.prepare(&assets);
        let verts: usize = w.chunks().map(|c| c.opaque.vertices.len()).sum();
        // 127 tops × 7 verts, plus edge walls.
        assert!(verts > 127 * 7);
        let before: usize = w.chunks().map(|c| c.opaque.indices.len()).sum();
        w.set_height(Hex::ORIGIN, 3);
        w.prepare(&assets);
        let after: usize = w.chunks().map(|c| c.opaque.indices.len()).sum();
        // Pointy hexes show 2 walls to the viewer (south-east and south-west faces; the east and
        // west faces are edge-on): × 3 levels × 2 triangles × 3 indices.
        assert_eq!(after - before, 2 * 3 * 6);
    }

    #[test]
    fn sized_wall_textures_tile_at_true_size() {
        let mut assets = Assets::new();
        let mut w = world();
        let cliff = assets.add(crate::Image::new(32, 16));
        w.materials[0].side_texture = Some(cliff);
        w.materials[0].side_texture_size = Some(Vec2::new(2.0, 1.0));
        w.prepare(&assets);
        let before: usize = w.chunks().map(|c| c.opaque.indices.len()).sum();
        // 6 levels × 0.35 = 2.1 units of wall: two whole copies of the 1-unit texture and a
        // 0.1 sliver, instead of one stretched copy per level.
        w.set_height(Hex::ORIGIN, 6);
        w.prepare(&assets);
        let after: usize = w.chunks().map(|c| c.opaque.indices.len()).sum();
        assert_eq!(after - before, 2 * 3 * 6);
        // Every wall vertex samples inside the texture (the atlas can't wrap).
        let r = assets.region(cliff);
        let inside = |uv: [f32; 2]| {
            (r.uv0[0] - 1e-5..=r.uv1[0] + 1e-5).contains(&uv[0]) && (r.uv0[1] - 1e-5..=r.uv1[1] + 1e-5).contains(&uv[1])
        };
        let walls = w
            .chunks()
            .flat_map(|c| c.opaque.vertices.iter())
            .filter(|v| v.uv[1] >= r.uv0[1] - 1e-5 && v.uv[0] >= r.uv0[0] - 1e-5 && v.uv[0] <= r.uv1[0] + 1e-5)
            .map(|v| v.uv)
            .collect::<Vec<_>>();
        assert!(walls.len() >= 2 * 3 * 4);
        assert!(walls.into_iter().all(inside));
    }

    #[test]
    fn picking_hits_the_hex_under_the_cursor() {
        let mut w = world();
        w.set_height(Hex::new(1, 0), 4);
        let mut cam = Camera::new(Vec3::ZERO, 40.0);
        cam.viewport = crate::Rect::new(0.0, 0.0, 800.0, 600.0);
        for h in [Hex::ORIGIN, Hex::new(-2, 1), Hex::new(1, 0), Hex::new(3, -3)] {
            let s = cam.world_to_screen(w.hex_to_world(h));
            let pick = w.pick(cam.screen_ray(s)).expect("hit");
            assert_eq!(pick.hex, h);
        }
        // The ground just behind (north of) the tall hex is hidden by it.
        let behind = w.hex_to_world(Hex::new(1, 0)) + Vec3::new(0.0, -w.ground_y(Hex::new(1, 0)), -1.2);
        let s = cam.world_to_screen(behind);
        assert_eq!(w.pick(cam.screen_ray(s)).unwrap().hex, Hex::new(1, 0));
    }

    #[test]
    fn water_and_fog() {
        let mut w = world();
        w.water = Some(Water { level: 0, color: Color::BLUE, texture: None });
        w.set_height(Hex::ORIGIN, -2);
        assert!(w.is_submerged(Hex::ORIGIN));
        assert_eq!(w.surface_y(Hex::ORIGIN), 0.0);
        w.set_fog_enabled(true);
        assert_eq!(w.visibility(Hex::ORIGIN), Visibility::Hidden);
        w.update_fog([Hex::ORIGIN]);
        assert!(w.is_visible(Hex::ORIGIN));
        w.update_fog([Hex::new(1, 0)]);
        assert_eq!(w.visibility(Hex::ORIGIN), Visibility::Explored);
        assert_eq!(w.visible_hexes().collect::<Vec<_>>(), vec![Hex::new(1, 0)]);
        // An unchanged visible set changes nothing (no chunk rebuilds, no version bump).
        let v = w.fog_version();
        w.update_fog([Hex::new(1, 0)]);
        assert_eq!(w.fog_version(), v);
        let assets = Assets::new();
        w.prepare(&assets);
        assert!(w.chunks().any(|c| !c.transparent.is_empty()));
        // Clearing the map forgets what was explored.
        w.clear();
        assert_eq!(w.visible_hexes().count(), 0);
        assert_eq!(w.visibility(Hex::new(1, 0)), Visibility::Hidden);
    }

    fn big_world(n: i32) -> HexWorld {
        let mut w = HexWorld::new(Layout::pointy(1.0));
        let noise = knight_hex::ValueNoise::new(3);
        for row in 0..n {
            for col in 0..n {
                let h = knight_hex::Offset::new(col, row).to_hex_r(knight_hex::Parity::Odd);
                w.set_tile(h, Tile::new((noise.fbm(col as f32 * 0.07, row as f32 * 0.07, 3) * 10.0) as i16 - 2, 0));
            }
        }
        w
    }

    /// The footprint search never misses anything on screen: every hex visible at any point of
    /// the viewport (found by picking a dense grid of screen points) is in a returned chunk.
    #[test]
    fn visible_chunk_keys_cover_the_screen() {
        let w = big_world(200);
        for (x, z, zoom, pitch) in [
            (50.0, 60.0, 30.0, 50.0),
            (170.0, 250.0, 12.0, 35.0),
            (300.0, 20.0, 60.0, 90.0),
            (-40.0, -40.0, 20.0, 50.0),
        ] {
            let mut cam = crate::Camera::new(Vec3::new(x, 0.0, z), zoom).with_pitch(pitch);
            cam.viewport = crate::Rect::new(0.0, 0.0, 1600.0, 900.0);
            let fast: HashSet<ChunkKey> = w.visible_chunk_keys(&cam).into_iter().collect();
            let mut seen = 0;
            for sx in (0..=1600).step_by(40) {
                for sy in (0..=900).step_by(40) {
                    if let Some(p) = w.pick(cam.screen_ray(Vec2::new(sx as f32, sy as f32))) {
                        seen += 1;
                        let k = HexWorld::chunk_of(p.hex);
                        assert!(
                            fast.contains(&k),
                            "view ({x}, {z}) zoom {zoom}: hex {:?} in chunk {k:?} was missed",
                            p.hex
                        );
                    }
                }
            }
            if x > 0.0 {
                assert!(seen > 0, "the view looks at the map");
            }
        }
    }

    /// Only visible chunks get meshed, a few per frame, and memory stays capped while flying.
    #[test]
    fn lazy_meshing_and_eviction() {
        let assets = Assets::new();
        let mut w = big_world(256);
        w.mesh_cache = 64;
        let mut cam = crate::Camera::new(Vec3::new(100.0, 0.0, 100.0), 32.0);
        cam.viewport = crate::Rect::new(0.0, 0.0, 1600.0, 900.0);
        let first = w.visible_chunks(&cam, &assets);
        assert!(!first.is_empty());
        let (meshed, _) = w.mesh_stats();
        assert_eq!(meshed, first.len(), "nothing off-screen was meshed");
        for step in 0..120 {
            cam.target.x = 100.0 + step as f32 * 2.0;
            w.visible_chunks(&cam, &assets);
            assert!(w.mesh_stats().0 <= 64.max(2 * w.visible_chunk_keys(&cam).len()));
        }
        // Editing a tile far away doesn't mesh anything; editing a visible one re-meshes it.
        let visible_hex = w.world_to_hex(cam.target);
        let before =
            w.visible_chunks(&cam, &assets).iter().find(|c| c.key == HexWorld::chunk_of(visible_hex)).unwrap().version;
        w.set_height(visible_hex, 9);
        w.set_height(Hex::new(-5000, -5000), 3);
        let after =
            w.visible_chunks(&cam, &assets).iter().find(|c| c.key == HexWorld::chunk_of(visible_hex)).unwrap().version;
        assert!(after > before, "the edited chunk was rebuilt");
    }

    /// A zoomed-out jump is meshed progressively (budget per frame), never all at once.
    #[test]
    fn mesh_budget_spreads_work() {
        let assets = Assets::new();
        let mut w = big_world(256);
        w.mesh_budget = 10;
        let mut cam = crate::Camera::new(Vec3::new(200.0, 0.0, 200.0), 4.0);
        cam.viewport = crate::Rect::new(0.0, 0.0, 1600.0, 900.0);
        let visible = w.visible_chunk_keys(&cam).len();
        assert!(visible > 30);
        let got = w.visible_chunks(&cam, &assets).len();
        assert_eq!(got, 10);
        let mut frames = 1;
        while w.visible_chunks(&cam, &assets).len() < visible {
            frames += 1;
            assert!(frames < 100);
        }
        // Exact bounds replace conservative ones as chunks get meshed, so the set can shrink a
        // little while it fills in.
        assert!(frames <= visible.div_ceil(10) + 1, "{frames} frames for {visible} chunks");
    }
}
