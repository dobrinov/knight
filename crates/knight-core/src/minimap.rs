//! Minimap as an image: one pixel per hex (down-sampled for huge maps), with height shading,
//! water and fog. Unlike a second camera it costs the same for a 50-hex or a million-hex map,
//! and it only re-renders when the terrain or fog changed (at most a few times per second).
//! Draw it as a UI image; convert clicks with [`Minimap::uv_to_world`].

use glam::{Vec2, Vec3};
use knight_hex::{Hex, Offset, Parity, Point};

use crate::assets::{Assets, ImageId};
use crate::world::{HexWorld, Visibility};
use crate::{Camera, Color, Image};

#[derive(Clone, Debug)]
pub struct Minimap {
    /// Largest image side in pixels (bigger maps are sampled every few hexes).
    pub max_size: u32,
    /// Seconds between refreshes when something changed.
    pub refresh: f32,
    image: Option<ImageId>,
    size: (u32, u32),
    origin: (i32, i32),
    step: i32,
    pointy: bool,
    seen: (u64, u64, usize),
    timer: f32,
    colors: Vec<[u8; 4]>,
}

impl Default for Minimap {
    fn default() -> Self {
        Minimap::new(256)
    }
}

impl Minimap {
    pub fn new(max_size: u32) -> Self {
        Minimap {
            max_size,
            refresh: 0.25,
            image: None,
            size: (0, 0),
            origin: (0, 0),
            step: 1,
            pointy: true,
            seen: (u64::MAX, u64::MAX, usize::MAX),
            timer: 0.0,
            colors: Vec::new(),
        }
    }

    /// The minimap image (after the first [`Minimap::update`]).
    pub fn image(&self) -> Option<ImageId> {
        self.image
    }

    /// Width / height ratio to draw the image with (hexes are not square).
    pub fn aspect(&self) -> f32 {
        let (w, h) = self.size;
        if h == 0 {
            return 1.0;
        }
        // Pointy rows are 1.5 units apart and √3 wide; flat columns the other way round.
        let (cw, ch) = if self.pointy { (1.732, 1.5) } else { (1.5, 1.732) };
        (w as f32 * cw) / (h as f32 * ch)
    }

    fn offset(&self, h: Hex) -> Offset {
        if self.pointy { Offset::from_hex_r(h, Parity::Odd) } else { Offset::from_hex_q(h, Parity::Odd) }
    }

    fn hex(&self, o: Offset) -> Hex {
        if self.pointy { o.to_hex_r(Parity::Odd) } else { o.to_hex_q(Parity::Odd) }
    }

    /// Average colour of an atlas image.
    fn average(assets: &Assets, image: ImageId) -> Color {
        let (atlas, r) = (assets.atlas(), assets.region(image));
        let mut sum = [0u64; 3];
        let mut n = 0u64;
        for y in (r.y..r.y + r.height).step_by(2) {
            for x in (r.x..r.x + r.width).step_by(2) {
                let px = atlas.pixel(r.page, x, y);
                for (c, s) in sum.iter_mut().enumerate() {
                    *s += px[c] as u64;
                }
                n += 1;
            }
        }
        let avg = |c: usize| (sum[c] / n.max(1)) as u8;
        Color::from([avg(0), avg(1), avg(2), 255])
    }

    /// Average colour of each material (textured ones sampled from the atlas).
    fn material_colors(world: &HexWorld, assets: &Assets) -> Vec<[u8; 4]> {
        world
            .materials
            .iter()
            .map(|m| match m.textures.first() {
                Some(&tex) => Self::average(assets, tex).mul_rgb(m.top).to_rgba8(),
                None => m.top.to_rgba8(),
            })
            .collect()
    }

    /// Re-render if the world or its fog changed (throttled). Call once per frame.
    pub fn update(&mut self, world: &HexWorld, assets: &mut Assets, dt: f32) -> Option<ImageId> {
        self.timer -= dt;
        let now = (world.version(), world.fog_version(), world.materials.len());
        if now == self.seen || (self.image.is_some() && self.timer > 0.0) {
            return self.image;
        }
        self.timer = self.refresh;
        let rebuild_layout = self.image.is_none() || now.0 != self.seen.0 || now.2 != self.seen.2;
        self.seen = now;
        if now.2 != self.colors.len() {
            self.colors = Self::material_colors(world, assets);
        }
        if rebuild_layout {
            self.pointy = world.layout.orientation.is_pointy();
            let (mut c0, mut r0, mut c1, mut r1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for h in world.hexes() {
                let o = self.offset(h);
                c0 = c0.min(o.col);
                r0 = r0.min(o.row);
                c1 = c1.max(o.col);
                r1 = r1.max(o.row);
            }
            if c0 > c1 {
                return self.image;
            }
            let (w, h) = ((c1 - c0 + 1) as u32, (r1 - r0 + 1) as u32);
            self.step = (w.max(h) as f32 / self.max_size as f32).ceil().max(1.0) as i32;
            let size = (w.div_ceil(self.step as u32), h.div_ceil(self.step as u32));
            if size != self.size {
                // A new size needs a new atlas image.
                self.image = None;
            }
            self.size = size;
            self.origin = (c0, r0);
        }
        let (w, h) = self.size;
        let water = world.water.as_ref();
        // Textured water is tinted by its colour; use the texture's average.
        let water_color = water.map(|w| match w.texture {
            Some(t) => Self::average(assets, t).mul_rgb(w.color),
            None => w.color,
        });
        let img = Image::from_fn(w, h, |x, y| {
            let o = Offset::new(self.origin.0 + x as i32 * self.step, self.origin.1 + y as i32 * self.step);
            let hex = self.hex(o);
            let Some(t) = world.tile(hex) else { return [0, 0, 0, 0] };
            let fog = match world.visibility(hex) {
                Visibility::Visible => 1.0,
                Visibility::Explored => 0.55,
                Visibility::Hidden => return [12, 12, 18, 255],
            };
            let base = match water {
                Some(wt) if world.is_submerged(hex) => {
                    let depth = (wt.level - t.height) as f32;
                    water_color.unwrap_or(wt.color).shade(1.1 - 0.08 * depth)
                }
                _ => Color::from(self.colors.get(t.material as usize).copied().unwrap_or([128, 128, 128, 255]))
                    .shade(0.82 + 0.035 * t.height as f32),
            };
            base.shade(fog).with_alpha(1.0).to_rgba8()
        });
        match self.image {
            Some(id) => assets.replace(id, &img),
            None => self.image = Some(assets.add(img)),
        }
        self.image
    }

    /// Minimap coordinates (0..1) of a world point.
    pub fn world_to_uv(&self, world: &HexWorld, p: Vec3) -> Vec2 {
        let o = self.offset(world.layout.point_to_hex(Point::new(p.x, p.z)));
        let (w, h) = self.size;
        Vec2::new(
            ((o.col - self.origin.0) as f32 / self.step as f32 + 0.5) / w.max(1) as f32,
            ((o.row - self.origin.1) as f32 / self.step as f32 + 0.5) / h.max(1) as f32,
        )
    }

    /// World point (on the ground) at minimap coordinates (0..1), e.g. to jump the camera.
    pub fn uv_to_world(&self, world: &HexWorld, uv: Vec2) -> Vec3 {
        let (w, h) = self.size;
        let col = self.origin.0 + (uv.x * w as f32 * self.step as f32) as i32;
        let row = self.origin.1 + (uv.y * h as f32 * self.step as f32) as i32;
        let hex = self.hex(Offset::new(col, row));
        let p = world.layout.hex_to_point(hex);
        Vec3::new(p.x, 0.0, p.y)
    }

    /// The camera's ground footprint as minimap coordinates (for drawing the view box).
    pub fn view_rect(&self, world: &HexWorld, camera: &Camera) -> Option<(Vec2, Vec2)> {
        let v = camera.viewport;
        let corners = [
            Vec2::new(v.x, v.y),
            Vec2::new(v.x + v.w, v.y),
            Vec2::new(v.x, v.y + v.h),
            Vec2::new(v.x + v.w, v.y + v.h),
        ];
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for c in corners {
            let g = camera.ground_at(c, 0.0)?;
            let uv = self.world_to_uv(world, g);
            lo = lo.min(uv);
            hi = hi.max(uv);
        }
        Some((lo.clamp(Vec2::ZERO, Vec2::ONE), hi.clamp(Vec2::ZERO, Vec2::ONE)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Material, Tile};
    use knight_hex::{Layout, shapes};

    #[test]
    fn renders_and_maps_coordinates() {
        let mut assets = Assets::new();
        let mut w = HexWorld::new(Layout::pointy(1.0));
        let m = w.add_material(Material::color("g", Color::GREEN, Color::GRAY));
        for h in shapes::rectangle(40, 30, true, Parity::Odd) {
            w.set_tile(h, Tile::new(1, m));
        }
        let mut mm = Minimap::new(16);
        let id = mm.update(&w, &mut assets, 0.0).unwrap();
        assert_eq!(assets.region(id).width, 14, "40 columns sampled every 3 hexes");
        let p = w.hex_to_world(Offset::new(30, 20).to_hex_r(Parity::Odd));
        let back = mm.uv_to_world(&w, mm.world_to_uv(&w, p));
        assert!(Vec2::new(back.x - p.x, back.z - p.z).length() < 6.0, "round trip within one sample");
        // Unchanged world: no re-render; changed: the same image is updated in place.
        let v = assets.atlas_version();
        mm.update(&w, &mut assets, 1.0);
        assert_eq!(assets.atlas_version(), v);
        w.set_height(Hex::ORIGIN, 5);
        assert_eq!(mm.update(&w, &mut assets, 1.0), Some(id));
        assert!(assets.atlas_updates_since(v).is_some_and(|r| r.len() == 1));
    }
}
