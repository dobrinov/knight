//! Orthographic "isometric" camera with pan and zoom.
//!
//! The view direction is fixed (no rotation): the camera always looks "north" (towards −z) from
//! above, at a configurable `pitch`. World axes: `x` right, `y` up, `z` towards the viewer. Hex
//! layouts map their 2D `(x, y)` onto the ground plane as `(x, z)`, so map "down" is towards the
//! camera and on screen. Because the viewer is always to the south, terrain meshing only builds
//! walls that face the camera.

use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};

use crate::geom::{Ray, Rect, smooth};

const EYE_DISTANCE: f32 = 500.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    /// Ground point in the middle of the viewport.
    pub target: Vec3,
    /// Angle above the horizon, radians (`π/2` = straight down). A per-game setting; change it
    /// with [`Camera::set_pitch`].
    pub pitch: f32,
    /// Logical pixels per world unit (multiplied by [`Camera::dpr`] on screen).
    pub zoom: f32,
    /// Device pixel ratio: physical pixels per logical pixel. Keeps the same framing on
    /// high-DPI screens.
    pub dpr: f32,
    /// Screen area (physical pixels) the camera renders into.
    pub viewport: Rect,

    /// Values the camera eases towards (set by `zoom_by`, `move_to`, ...).
    pub goal_pitch: f32,
    pub goal_zoom: f32,
    pub goal_target: Option<Vec3>,
    /// Easing speed (higher = snappier).
    pub smoothing: f32,

    pub min_zoom: f32,
    pub max_zoom: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    /// Clamp the target to this ground rectangle (`x`/`z`).
    pub bounds: Option<(Vec2, Vec2)>,
    /// When set, the rendered view is snapped to this many physical pixels (pixel-art mode), so
    /// scrolling never shimmers.
    pub pixel_snap: Option<f32>,
    /// When set, zoom goals snap to pixel-perfect levels of this unit, in logical pixels (use
    /// `pixel_scale * art_pixels_per_unit / dpr`, so sprites are drawn at ¼, ½, 1, 2... × size).
    pub zoom_step: Option<f32>,

    zoom_anchor: Option<(Vec2, Vec3)>,
}

impl Default for Camera {
    fn default() -> Self {
        Camera::new(Vec3::ZERO, 48.0)
    }
}

impl Camera {
    pub fn new(target: Vec3, zoom: f32) -> Self {
        let pitch = 50f32.to_radians();
        Camera {
            target,
            pitch,
            zoom,
            dpr: 1.0,
            viewport: Rect::new(0.0, 0.0, 1280.0, 720.0),
            goal_pitch: pitch,
            goal_zoom: zoom,
            goal_target: None,
            smoothing: 14.0,
            min_zoom: 4.0,
            max_zoom: 400.0,
            min_pitch: 25f32.to_radians(),
            max_pitch: 90f32.to_radians(),
            bounds: None,
            pixel_snap: None,
            zoom_step: None,
            zoom_anchor: None,
        }
    }

    /// Jump straight to the goals (no easing).
    pub fn snap(&mut self) {
        self.pitch = self.goal_pitch;
        self.zoom = self.goal_zoom;
        if let Some(t) = self.goal_target.take() {
            self.target = t;
        }
        self.zoom_anchor = None;
    }

    pub fn with_pitch(mut self, degrees: f32) -> Self {
        self.pitch = degrees.to_radians();
        self.goal_pitch = self.pitch;
        self
    }

    /// Camera basis: (right, up, back). `back` points from the target towards the eye.
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let (sp, cp) = self.pitch.sin_cos();
        let right = Vec3::X;
        let back = Vec3::new(0.0, sp, cp);
        let up = back.cross(right);
        (right, up, back)
    }

    /// Horizontal unit vector pointing "into the screen" (up on screen, projected on the ground).
    pub fn forward_ground(&self) -> Vec3 {
        Vec3::NEG_Z
    }

    fn render_target(&self) -> Vec3 {
        let Some(px) = self.pixel_snap else { return self.target };
        let (r, u, _) = self.basis();
        let step = px / self.scale();
        let x = self.target.dot(r);
        let y = self.target.dot(u);
        self.target + r * ((x / step).round() * step - x) + u * ((y / step).round() * step - y)
    }

    pub fn view(&self) -> Mat4 {
        let (r, u, b) = self.basis();
        let eye = self.render_target() + b * EYE_DISTANCE;
        Mat4::from_cols(
            Vec4::new(r.x, u.x, b.x, 0.0),
            Vec4::new(r.y, u.y, b.y, 0.0),
            Vec4::new(r.z, u.z, b.z, 0.0),
            Vec4::new(-r.dot(eye), -u.dot(eye), -b.dot(eye), 1.0),
        )
    }

    /// Physical pixels per world unit.
    pub fn scale(&self) -> f32 {
        self.zoom * self.dpr
    }

    pub fn projection(&self) -> Mat4 {
        let hw = self.viewport.w / self.scale() * 0.5;
        let hh = self.viewport.h / self.scale() * 0.5;
        glam::camera::rh::proj::directx::orthographic(-hw, hw, -hh, hh, 1.0, EYE_DISTANCE * 2.0)
    }

    pub fn view_proj(&self) -> Mat4 {
        self.projection() * self.view()
    }

    /// Project a world point to physical screen pixels.
    pub fn world_to_screen(&self, p: Vec3) -> Vec2 {
        let c = self.view_proj() * p.extend(1.0);
        self.ndc_to_screen(c.xy() / c.w)
    }

    /// Clip-space depth of a world point (0 = near, 1 = far).
    pub fn depth_of(&self, p: Vec3) -> f32 {
        let c = self.view_proj() * p.extend(1.0);
        c.z / c.w
    }

    fn ndc_to_screen(&self, n: Vec2) -> Vec2 {
        Vec2::new(
            self.viewport.x + (n.x + 1.0) * 0.5 * self.viewport.w,
            self.viewport.y + (1.0 - n.y) * 0.5 * self.viewport.h,
        )
    }

    fn screen_to_ndc(&self, s: Vec2) -> Vec2 {
        Vec2::new(
            (s.x - self.viewport.x) / self.viewport.w * 2.0 - 1.0,
            1.0 - (s.y - self.viewport.y) / self.viewport.h * 2.0,
        )
    }

    /// The world ray through a screen pixel, pointing away from the viewer.
    pub fn screen_ray(&self, s: Vec2) -> Ray {
        let inv = self.view_proj().inverse();
        let n = self.screen_to_ndc(s);
        let near = inv * Vec4::new(n.x, n.y, 0.0, 1.0);
        let far = inv * Vec4::new(n.x, n.y, 1.0, 1.0);
        let near = near.xyz() / near.w;
        let far = far.xyz() / far.w;
        Ray { origin: near, dir: (far - near).normalize() }
    }

    /// Point on the horizontal plane `y = height` under a screen pixel.
    pub fn ground_at(&self, s: Vec2, height: f32) -> Option<Vec3> {
        self.screen_ray(s).hit_plane_y(height)
    }

    /// Move the view by a screen-space drag (physical pixels), keeping the ground under the
    /// cursor.
    pub fn pan_screen(&mut self, delta: Vec2) {
        let (r, _, _) = self.basis();
        let f = self.forward_ground();
        let sp = self.pitch.sin().max(0.2);
        self.target += -r * delta.x / self.scale() + f * delta.y / (self.scale() * sp);
        self.goal_target = None;
        self.clamp();
    }

    /// Move the target by a ground-space offset relative to the view (x = right, y = into screen).
    pub fn pan_ground(&mut self, delta: Vec2) {
        let (r, _, _) = self.basis();
        self.target += r * delta.x + self.forward_ground() * delta.y;
        self.goal_target = None;
        self.clamp();
    }

    /// Ease the target to a world point.
    pub fn move_to(&mut self, p: Vec3) {
        self.goal_target = Some(p);
    }

    /// Multiply the zoom goal, keeping the ground under `anchor` (physical pixels) fixed. With
    /// [`Camera::zoom_step`] set, any factor above/below 1 moves one pixel-perfect level.
    pub fn zoom_by(&mut self, factor: f32, anchor: Option<Vec2>) {
        if let Some(unit) = self.zoom_step {
            let levels = Self::zoom_levels(unit);
            let cur = levels
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1 - self.goal_zoom).abs().total_cmp(&(b.1 - self.goal_zoom).abs()))
                .map_or(0, |(i, _)| i);
            let next = if factor > 1.0 {
                (cur + 1).min(levels.len() - 1)
            } else if factor < 1.0 {
                cur.saturating_sub(1)
            } else {
                cur
            };
            self.set_zoom(levels[next], anchor);
        } else {
            self.set_zoom(self.goal_zoom * factor, anchor);
        }
    }

    /// Pixel-perfect zoom levels for a given step: ¼×, ½×, 1×, 2×, 3×, 4×, 6×, 8×.
    pub fn zoom_levels(unit: f32) -> [f32; 8] {
        [0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0].map(|k| k * unit)
    }

    pub fn set_zoom(&mut self, zoom: f32, anchor: Option<Vec2>) {
        let mut z = zoom.clamp(self.min_zoom, self.max_zoom);
        if let Some(unit) = self.zoom_step {
            // Keep art at whole-pixel scales: snap to the nearest level within the limits.
            let levels = Self::zoom_levels(unit);
            let ok: Vec<f32> = levels.iter().copied().filter(|l| *l >= self.min_zoom && *l <= self.max_zoom).collect();
            let pool = if ok.is_empty() { levels.to_vec() } else { ok };
            z = pool.into_iter().min_by(|a, b| (a / z).ln().abs().total_cmp(&(b / z).ln().abs())).unwrap_or(z);
        }
        self.goal_zoom = z;
        self.zoom_anchor = anchor.and_then(|s| self.ground_at(s, self.target.y).map(|g| (s, g)));
    }

    /// Ease to a new viewing angle (degrees above the horizon, 90 = top-down).
    pub fn set_pitch(&mut self, degrees: f32) {
        self.goal_pitch = degrees.to_radians().clamp(self.min_pitch, self.max_pitch);
    }

    /// Advance easing. Call once per frame.
    pub fn update(&mut self, dt: f32) {
        let k = smooth(self.smoothing, dt);
        self.pitch += (self.goal_pitch - self.pitch) * k;
        let old_zoom = self.zoom;
        self.zoom = (self.zoom.ln() + (self.goal_zoom.ln() - self.zoom.ln()) * k).exp();
        if (self.zoom - self.goal_zoom).abs() < 0.01 {
            self.zoom = self.goal_zoom;
        }
        if let Some((screen, ground)) = self.zoom_anchor {
            if old_zoom != self.zoom
                && let Some(now) = self.ground_at(screen, self.target.y)
            {
                self.target += ground - now;
            }
            if self.zoom == self.goal_zoom {
                self.zoom_anchor = None;
            }
        }
        if let Some(goal) = self.goal_target {
            self.target += (goal - self.target) * k;
            if self.target.distance(goal) < 1e-3 {
                self.target = goal;
                self.goal_target = None;
            }
        }
        self.clamp();
    }

    fn clamp(&mut self) {
        if let Some((min, max)) = self.bounds {
            self.target.x = self.target.x.clamp(min.x, max.x);
            self.target.z = self.target.z.clamp(min.y, max.y);
        }
    }

    /// Is any part of a world-space box inside the view?
    pub fn sees_box(&self, min: Vec3, max: Vec3) -> bool {
        Self::box_in_view(&self.view_proj(), min, max)
    }

    /// [`Camera::sees_box`] with a precomputed [`Camera::view_proj`] (for testing many boxes).
    pub fn box_in_view(vp: &Mat4, min: Vec3, max: Vec3) -> bool {
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for i in 0..8 {
            let p = Vec3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { min.y } else { max.y },
                if i & 4 == 0 { min.z } else { max.z },
            );
            let c = vp * p.extend(1.0);
            let n = c.xy() / c.w;
            lo = lo.min(n);
            hi = hi.max(n);
        }
        hi.x >= -1.0 && lo.x <= 1.0 && hi.y >= -1.0 && lo.y <= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> Camera {
        let mut c = Camera::new(Vec3::new(3.0, 0.0, 2.0), 40.0);
        c.viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        c
    }

    #[test]
    fn target_projects_to_viewport_centre() {
        let c = cam();
        let s = c.world_to_screen(c.target);
        assert!((s - Vec2::new(400.0, 300.0)).length() < 1e-3);
    }

    #[test]
    fn screen_ray_roundtrip() {
        let c = cam();
        for p in [Vec3::new(1.0, 0.0, 1.0), Vec3::new(-4.0, 0.0, 7.5)] {
            let s = c.world_to_screen(p);
            let g = c.ground_at(s, 0.0).unwrap();
            assert!((g - p).length() < 1e-3, "{g} vs {p}");
        }
    }

    #[test]
    fn pan_keeps_ground_under_cursor() {
        let mut c = cam();
        let p = Vec3::new(2.0, 0.0, 5.0);
        let before = c.world_to_screen(p);
        c.pan_screen(Vec2::new(37.0, -21.0));
        let after = c.world_to_screen(p);
        assert!((after - before - Vec2::new(37.0, -21.0)).length() < 1e-2);
    }

    #[test]
    fn zoom_anchor_is_stable() {
        let mut c = cam();
        let anchor = Vec2::new(100.0, 120.0);
        let ground = c.ground_at(anchor, 0.0).unwrap();
        c.zoom_by(2.0, Some(anchor));
        for _ in 0..200 {
            c.update(1.0 / 60.0);
        }
        assert!((c.zoom - 80.0).abs() < 1e-3);
        assert!((c.ground_at(anchor, 0.0).unwrap() - ground).length() < 1e-2);
    }

    #[test]
    fn top_down_camera_looks_down() {
        let c = cam().with_pitch(90.0);
        let (r, u, b) = c.basis();
        assert!((b - Vec3::Y).length() < 1e-5);
        assert!((r - Vec3::X).length() < 1e-5);
        assert!((u - Vec3::NEG_Z).length() < 1e-5);
    }
}
