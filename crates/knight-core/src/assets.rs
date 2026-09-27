//! Asset registry. All images live in one texture atlas so a whole frame draws with a handful of
//! draw calls; the runtime re-uploads the atlas whenever [`Assets::atlas_version`] changes.
//!
//! The atlas has pages of 2048² (the size every GPU, WebGL2 included, supports), stored as one
//! texture array. A region's page is the whole-number part of its U coordinate (`uv.x` runs
//! from `page` to `page + 1`), so vertices need no extra field; shaders split it back out.

use std::collections::HashMap;

use crate::{Color, Image};

/// Handle to an image (or animation frame) in the atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageId(pub u32);

/// Where an image lives in the atlas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    /// Top-left and bottom-right UVs (`uv.x` includes the page number, see the module docs).
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    /// Size in pixels.
    pub width: u32,
    pub height: u32,
    /// Atlas page and top-left pixel on it.
    pub page: u32,
    pub x: u32,
    pub y: u32,
}

impl Region {
    /// UV at a normalised position inside the region.
    pub fn uv(&self, u: f32, v: f32) -> [f32; 2] {
        [self.uv0[0] + (self.uv1[0] - self.uv0[0]) * u, self.uv0[1] + (self.uv1[1] - self.uv0[1]) * v]
    }
}

/// An animation: a list of frames played at a fixed rate.
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub frames: Vec<ImageId>,
    pub fps: f32,
    pub looping: bool,
}

impl Animation {
    pub fn new(frames: Vec<ImageId>, fps: f32, looping: bool) -> Self {
        Animation { frames, fps, looping }
    }

    /// Frame to show `t` seconds after the animation started.
    pub fn frame_at(&self, t: f32) -> ImageId {
        let n = self.frames.len().max(1);
        let i = (t.max(0.0) * self.fps) as usize;
        let i = if self.looping { i % n } else { i.min(n - 1) };
        self.frames[i]
    }

    pub fn duration(&self) -> f32 {
        self.frames.len() as f32 / self.fps.max(0.001)
    }

    pub fn finished(&self, t: f32) -> bool {
        !self.looping && t >= self.duration()
    }
}

const ATLAS_SIZE: u32 = 2048;
/// Most pages the atlas may grow to (16 × 16 MB).
pub const MAX_ATLAS_PAGES: u32 = 16;
const PAD: u32 = 2;

/// Shelf-packed RGBA atlas pages with edge extrusion, so linear filtering never bleeds
/// neighbours in. A new page opens when an image no longer fits on the current one.
pub struct Atlas {
    pub size: u32,
    pages: Vec<Vec<u8>>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
}

impl Atlas {
    fn new(size: u32) -> Self {
        Atlas { size, pages: vec![vec![0; (size * size * 4) as usize]], shelf_x: 0, shelf_y: 0, shelf_h: 0 }
    }

    fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32, u32)> {
        let (pw, ph) = (w + 2 * PAD, h + 2 * PAD);
        if pw > self.size || ph > self.size {
            return None;
        }
        if self.shelf_x + pw > self.size {
            self.shelf_y += self.shelf_h;
            self.shelf_x = 0;
            self.shelf_h = 0;
        }
        if self.shelf_y + ph > self.size {
            if self.pages.len() as u32 >= MAX_ATLAS_PAGES {
                return None;
            }
            self.pages.push(vec![0; (self.size * self.size * 4) as usize]);
            (self.shelf_x, self.shelf_y, self.shelf_h) = (0, 0, 0);
        }
        let pos = (self.pages.len() as u32 - 1, self.shelf_x + PAD, self.shelf_y + PAD);
        self.shelf_x += pw;
        self.shelf_h = self.shelf_h.max(ph);
        Some(pos)
    }

    fn write(&mut self, img: &Image, page: u32, x: u32, y: u32) {
        let size = self.size as i64;
        let pixels = &mut self.pages[page as usize];
        let pad = PAD as i64;
        for iy in -pad..img.height as i64 + pad {
            for ix in -pad..img.width as i64 + pad {
                let (px, py) = (x as i64 + ix, y as i64 + iy);
                if px < 0 || py < 0 || px >= size || py >= size {
                    continue;
                }
                let sx = ix.clamp(0, img.width as i64 - 1) as u32;
                let sy = iy.clamp(0, img.height as i64 - 1) as u32;
                let i = ((py * size + px) * 4) as usize;
                pixels[i..i + 4].copy_from_slice(&img.get(sx, sy));
            }
        }
    }

    /// Number of pages in use.
    pub fn page_count(&self) -> u32 {
        self.pages.len() as u32
    }

    /// RGBA pixels of one page (`size × size × 4` bytes).
    pub fn page(&self, page: u32) -> &[u8] {
        &self.pages[page as usize]
    }

    /// RGBA of one pixel.
    pub fn pixel(&self, page: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.size + x) * 4) as usize;
        let p = &self.pages[page as usize];
        [p[i], p[i + 1], p[i + 2], p[i + 3]]
    }

    /// Pages in use, counting the current one by its filled rows (e.g. 1.5 = one and a half).
    pub fn usage(&self) -> f32 {
        (self.pages.len() - 1) as f32 + (self.shelf_y + self.shelf_h) as f32 / self.size as f32
    }
}

struct AnimatedImage {
    id: ImageId,
    frames: Vec<Image>,
    fps: f32,
    shown: usize,
}

/// Built-in images every game gets.
#[derive(Clone, Copy, Debug)]
pub struct BuiltinImages {
    /// A single white pixel, used for untextured geometry.
    pub white: ImageId,
    /// A soft round blob (shadows, selection rings, particles).
    pub circle: ImageId,
    /// A ring outline.
    pub ring: ImageId,
    /// First glyph of the 8×8 ASCII font (glyph for char `c` is `font + (c - 32)`).
    pub font: ImageId,
}

/// All loaded images, animations and named lookups.
pub struct Assets {
    atlas: Atlas,
    regions: Vec<Region>,
    names: HashMap<String, ImageId>,
    animations: HashMap<String, Animation>,
    version: u64,
    /// Version of the last change that needs a full atlas upload (new images).
    full_version: u64,
    /// Recent in-place image updates: (version, [page, x, y, w, h] in atlas pixels).
    updates: Vec<(u64, [u32; 5])>,
    /// Images whose pixels cycle through frames (animated terrain, water).
    animated: Vec<AnimatedImage>,
    pub builtin: BuiltinImages,
}

impl Default for Assets {
    fn default() -> Self {
        Self::new()
    }
}

impl Assets {
    pub fn new() -> Self {
        let mut a = Assets {
            atlas: Atlas::new(ATLAS_SIZE),
            regions: Vec::new(),
            names: HashMap::new(),
            animations: HashMap::new(),
            version: 1,
            full_version: 1,
            updates: Vec::new(),
            animated: Vec::new(),
            builtin: BuiltinImages { white: ImageId(0), circle: ImageId(0), ring: ImageId(0), font: ImageId(0) },
        };
        let white = a.add(Image::filled(4, 4, Color::WHITE));
        let circle = a.add(Image::from_fn(64, 64, |x, y| {
            let d = ((x as f32 - 31.5).powi(2) + (y as f32 - 31.5).powi(2)).sqrt() / 32.0;
            let alpha = (1.0 - d).clamp(0.0, 1.0).powf(0.6);
            [255, 255, 255, (alpha * 255.0) as u8]
        }));
        let ring = a.add(Image::from_fn(64, 64, |x, y| {
            let d = ((x as f32 - 31.5).powi(2) + (y as f32 - 31.5).powi(2)).sqrt();
            let alpha = (1.0 - ((d - 28.5).abs() / 2.5)).clamp(0.0, 1.0);
            [255, 255, 255, (alpha * 255.0) as u8]
        }));
        let mut font = None;
        for c in 32..128usize {
            let glyph = font8x8::legacy::BASIC_LEGACY[c];
            let id = a.add(Image::from_fn(8, 8, |x, y| {
                if glyph[y as usize] & (1 << x) != 0 { [255; 4] } else { [255, 255, 255, 0] }
            }));
            font.get_or_insert(id);
        }
        a.builtin = BuiltinImages { white, circle, ring, font: font.unwrap() };
        a
    }

    /// Add an image to the atlas. Panics if the atlas is full or the image is bigger than a page.
    pub fn add(&mut self, img: Image) -> ImageId {
        let (page, x, y) = self
            .atlas
            .alloc(img.width, img.height)
            .unwrap_or_else(|| panic!("texture atlas full while adding a {}x{} image", img.width, img.height));
        self.atlas.write(&img, page, x, y);
        let s = self.atlas.size as f32;
        let p = page as f32;
        self.regions.push(Region {
            uv0: [p + x as f32 / s, y as f32 / s],
            uv1: [p + (x + img.width) as f32 / s, (y + img.height) as f32 / s],
            width: img.width,
            height: img.height,
            page,
            x,
            y,
        });
        self.version += 1;
        self.full_version = self.version;
        ImageId(self.regions.len() as u32 - 1)
    }

    /// Add an image and register it under `name`.
    pub fn add_named(&mut self, name: &str, img: Image) -> ImageId {
        let id = self.add(img);
        self.names.insert(name.to_string(), id);
        id
    }

    /// Decode and add a PNG.
    pub fn load_png(&mut self, name: &str, bytes: &[u8]) -> Result<ImageId, crate::image::ImageError> {
        Ok(self.add_named(name, Image::from_png(bytes)?))
    }

    /// Split a horizontal strip into frames and register them as an animation named `name`.
    pub fn add_strip(&mut self, name: &str, strip: &Image, frame_width: u32, fps: f32, looping: bool) -> Animation {
        let frames = strip.split_frames(frame_width).into_iter().map(|f| self.add(f)).collect();
        let anim = Animation::new(frames, fps, looping);
        self.animations.insert(name.to_string(), anim.clone());
        anim
    }

    /// Replace an image's pixels in place (same size). Only that rectangle is re-uploaded to
    /// the GPU, so it is cheap enough for per-frame changes like minimaps.
    pub fn replace(&mut self, id: ImageId, img: &Image) {
        let r = self.regions[id.0 as usize];
        Self::write_in_place(&mut self.atlas, &mut self.updates, &mut self.version, r, img);
    }

    /// [`Assets::replace`] on the parts it needs, so callers holding another field of `self`
    /// (like the animated frames) can use it too.
    fn write_in_place(
        atlas: &mut Atlas,
        updates: &mut Vec<(u64, [u32; 5])>,
        version: &mut u64,
        r: Region,
        img: &Image,
    ) {
        assert!(img.width == r.width && img.height == r.height, "replace() needs an image of the same size");
        atlas.write(img, r.page, r.x, r.y);
        *version += 1;
        updates.push((*version, [r.page, r.x - PAD, r.y - PAD, img.width + 2 * PAD, img.height + 2 * PAD]));
        if updates.len() > 256 {
            updates.drain(..128);
        }
    }

    /// Add an image whose pixels cycle through `frames` (all the same size) at `fps`. Use it
    /// like any other image: terrain textures and water keep their meshes while the pixels
    /// change, because [`Assets::update`] rewrites them in place (a cheap partial upload).
    pub fn add_animated(&mut self, frames: Vec<Image>, fps: f32) -> ImageId {
        assert!(!frames.is_empty(), "an animated image needs frames");
        let id = self.add(frames[0].clone());
        self.animated.push(AnimatedImage { id, frames, fps, shown: 0 });
        id
    }

    /// Advance animated images to `time` seconds. The runtime calls this every frame.
    pub fn update(&mut self, time: f64) {
        for a in &mut self.animated {
            let frame = (time * a.fps as f64) as usize % a.frames.len();
            if frame != a.shown {
                a.shown = frame;
                let r = self.regions[a.id.0 as usize];
                Self::write_in_place(&mut self.atlas, &mut self.updates, &mut self.version, r, &a.frames[frame]);
            }
        }
    }

    /// Atlas rectangles (`[page, x, y, w, h]`) changed since `version`, or `None` if a full
    /// upload is needed.
    pub fn atlas_updates_since(&self, version: u64) -> Option<Vec<[u32; 5]>> {
        if self.full_version > version {
            return None;
        }
        // Every version after the last full change is a logged update, unless the log was
        // trimmed past `version`.
        if self.updates.first().is_some_and(|(v, _)| *v > version + 1) {
            return None;
        }
        Some(self.updates.iter().filter(|(v, _)| *v > version).map(|(_, r)| *r).collect())
    }

    pub fn image(&self, name: &str) -> Option<ImageId> {
        self.names.get(name).copied()
    }

    pub fn animation(&self, name: &str) -> Option<&Animation> {
        self.animations.get(name)
    }

    pub fn region(&self, id: ImageId) -> Region {
        self.regions[id.0 as usize]
    }

    pub fn glyph(&self, c: char) -> ImageId {
        let c = c as u32;
        let c = if (32..128).contains(&c) { c } else { '?' as u32 };
        ImageId(self.builtin.font.0 + c - 32)
    }

    pub fn atlas(&self) -> &Atlas {
        &self.atlas
    }

    /// Bumped whenever atlas pixels change.
    pub fn atlas_version(&self) -> u64 {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_and_regions() {
        let mut a = Assets::new();
        let r = a.region(a.builtin.white);
        assert!(r.uv1[0] > r.uv0[0]);
        let id = a.add_named("x", Image::filled(10, 20, Color::RED));
        assert_eq!(a.image("x"), Some(id));
        assert_eq!(a.region(id).height, 20);
        assert_eq!(a.glyph('A').0, a.builtin.font.0 + 33);
        let anim = a.add_strip("walk", &Image::new(64, 16), 16, 8.0, true);
        assert_eq!(anim.frames.len(), 4);
        assert_eq!(anim.frame_at(0.26), anim.frames[2]);
        assert_eq!(anim.frame_at(0.5), anim.frames[0]);
    }

    #[test]
    fn replace_logs_partial_updates() {
        let mut a = Assets::new();
        let id = a.add(Image::filled(8, 8, Color::RED));
        let v = a.atlas_version();
        assert_eq!(a.atlas_updates_since(v - 1), None, "a new image needs a full upload");
        a.replace(id, &Image::filled(8, 8, Color::BLUE));
        let rects = a.atlas_updates_since(v).unwrap();
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0][3], 8 + 2 * PAD);
        assert_eq!(a.region(id).width, 8);
    }

    #[test]
    fn animated_images_rewrite_in_place() {
        let mut a = Assets::new();
        let id = a.add_animated(vec![Image::filled(4, 4, Color::RED), Image::filled(4, 4, Color::BLUE)], 2.0);
        let r = a.region(id);
        let v = a.atlas_version();
        a.update(0.2);
        assert_eq!(a.atlas_version(), v, "still frame 0");
        a.update(0.6);
        assert_eq!(a.atlas().pixel(r.page, r.x, r.y), Color::BLUE.to_rgba8());
        assert_eq!(a.atlas_updates_since(v).map(|u| u.len()), Some(1));
    }

    #[test]
    fn atlas_grows_pages() {
        let mut a = Assets::new();
        let big = Image::filled(1500, 1500, Color::RED);
        let first = a.add(big.clone());
        let second = a.add(big);
        let (r1, r2) = (a.region(first), a.region(second));
        assert_eq!((r1.page, r2.page), (0, 1));
        assert_eq!(a.atlas().page_count(), 2);
        // The page is the whole part of U; within the page U stays in 0..1.
        assert_eq!(r2.uv0[0].floor(), 1.0);
        assert!(r2.uv1[0] < 2.0);
        assert_eq!(a.atlas().pixel(1, r2.x, r2.y), Color::RED.to_rgba8());
    }
}
