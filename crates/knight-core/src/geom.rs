//! Small geometry helpers.

use glam::{Vec2, Vec3};

/// An axis-aligned rectangle (`x`, `y` is the top-left corner).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn from_corners(a: Vec2, b: Vec2) -> Rect {
        let min = a.min(b);
        let max = a.max(b);
        Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
    }

    pub fn min(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    pub fn max(&self) -> Vec2 {
        Vec2::new(self.x + self.w, self.y + self.h)
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }

    pub fn size(&self) -> Vec2 {
        Vec2::new(self.w, self.h)
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.w && p.y < self.y + self.h
    }

    /// Shrink on every side by `d` (negative grows).
    pub fn inset(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2.0 * d).max(0.0), (self.h - 2.0 * d).max(0.0))
    }

    pub fn scale(&self, k: f32) -> Rect {
        Rect::new(self.x * k, self.y * k, self.w * k, self.h * k)
    }

    pub fn translate(&self, d: Vec2) -> Rect {
        Rect::new(self.x + d.x, self.y + d.y, self.w, self.h)
    }

    /// Split into `n` equal rows.
    pub fn rows(&self, n: usize, gap: f32) -> Vec<Rect> {
        let n = n.max(1);
        let h = (self.h - gap * (n - 1) as f32) / n as f32;
        (0..n).map(|i| Rect::new(self.x, self.y + i as f32 * (h + gap), self.w, h)).collect()
    }

    /// Split into `n` equal columns.
    pub fn cols(&self, n: usize, gap: f32) -> Vec<Rect> {
        let n = n.max(1);
        let w = (self.w - gap * (n - 1) as f32) / n as f32;
        (0..n).map(|i| Rect::new(self.x + i as f32 * (w + gap), self.y, w, self.h)).collect()
    }
}

/// A ray in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
}

impl Ray {
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.dir * t
    }

    /// Intersection with the horizontal plane `y = height`.
    pub fn hit_plane_y(&self, height: f32) -> Option<Vec3> {
        if self.dir.y.abs() < 1e-6 {
            return None;
        }
        let t = (height - self.origin.y) / self.dir.y;
        (t >= 0.0).then(|| self.at(t))
    }
}

/// Exponential smoothing factor for frame-rate independent easing.
pub fn smooth(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// Shortest signed difference between two angles (radians).
pub fn angle_diff(from: f32, to: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (to - from + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}
