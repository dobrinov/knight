//! Standard camera controls: drag to pan, wheel / pinch to zoom, keyboard pan, edge scrolling.

use glam::Vec2;

use crate::{Camera, Input, Key, MouseButton};

/// Within this many logical pixels of the edge, edge-scrolling works even over UI panels.
pub const EDGE_OVER_UI: f32 = 10.0;

#[derive(Clone, Debug)]
pub struct CameraController {
    /// Buttons that pan when dragged.
    pub drag_buttons: Vec<MouseButton>,
    /// WASD / arrow keys pan; +/- zoom.
    pub keyboard: bool,
    /// Keyboard pan speed in logical pixels per second.
    pub key_speed: f32,
    pub wheel_zoom: bool,
    /// Zoom factor per wheel notch.
    pub zoom_factor: f32,
    /// Pinch-zoom and two-finger pan.
    pub touch: bool,
    /// Pan when the mouse is within this many logical pixels of the viewport edge (0 = off).
    /// Speed ramps up the closer the pointer is to the edge (StarCraft-style). Over UI panels
    /// only the outermost [`EDGE_OVER_UI`] pixels pan; nothing pans when the pointer leaves the
    /// window.
    pub edge_scroll: f32,
    /// Edge-scroll speed at the very edge, in logical pixels per second.
    pub edge_speed: f32,
    /// Accumulated zoom (log scale) for cameras with pixel-perfect zoom levels.
    zoom_acc: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        CameraController {
            drag_buttons: vec![MouseButton::Left, MouseButton::Right, MouseButton::Middle],
            keyboard: true,
            key_speed: 700.0,
            wheel_zoom: true,
            zoom_factor: 1.15,
            touch: true,
            edge_scroll: 40.0,
            edge_speed: 1500.0,
            zoom_acc: 0.0,
        }
    }
}

impl CameraController {
    /// RTS-style: left button is for selection, so only right / middle drag pans.
    pub fn rts() -> Self {
        Self::default().drag_with(&[MouseButton::Middle, MouseButton::Right])
    }

    /// Choose which mouse buttons pan when dragged.
    pub fn drag_with(mut self, buttons: &[MouseButton]) -> Self {
        self.drag_buttons = buttons.to_vec();
        self
    }

    /// Apply controls to `cam`, then advance its easing. `over_ui` suppresses pointer controls.
    pub fn update(&mut self, cam: &mut Camera, input: &Input, dt: f32, over_ui: bool) {
        let pointer_in_view = cam.viewport.contains(input.mouse);
        let dragging = self.drag_buttons.iter().any(|&b| input.drag(b).is_some());
        if dragging && input.touch_count() < 2 {
            cam.pan_screen(input.mouse_delta);
        }
        if !over_ui && pointer_in_view {
            let mut factor = 1.0;
            if self.wheel_zoom && input.wheel != 0.0 {
                factor *= self.zoom_factor.powf(input.wheel);
            }
            if self.touch && input.pinch != 1.0 {
                factor *= input.pinch;
            }
            if factor != 1.0 {
                if cam.zoom_step.is_some() {
                    // Discrete levels: step once enough wheel / pinch has accumulated.
                    self.zoom_acc += factor.ln();
                    if self.zoom_acc.abs() >= 1.2f32.ln() {
                        cam.zoom_by(if self.zoom_acc > 0.0 { 2.0 } else { 0.5 }, Some(input.mouse));
                        self.zoom_acc = 0.0;
                    }
                } else {
                    cam.zoom_by(factor, Some(input.mouse));
                }
            }
        }
        if self.touch && input.two_finger_pan != Vec2::ZERO {
            cam.pan_screen(input.two_finger_pan);
        }
        // Cmd/Ctrl + key is a shortcut, never a pan.
        if self.keyboard && input.focused && !input.ctrl() {
            let mut d = Vec2::ZERO;
            if input.key_down(Key::A) || input.key_down(Key::Left) {
                d.x -= 1.0;
            }
            if input.key_down(Key::D) || input.key_down(Key::Right) {
                d.x += 1.0;
            }
            if input.key_down(Key::W) || input.key_down(Key::Up) {
                d.y -= 1.0;
            }
            if input.key_down(Key::S) || input.key_down(Key::Down) {
                d.y += 1.0;
            }
            if d != Vec2::ZERO {
                cam.pan_screen(-d.normalize() * self.key_speed * cam.dpr.max(1.0) * dt);
            }
            let center = Some(cam.viewport.center());
            if input.key_pressed(Key::Equal) {
                cam.zoom_by(1.25, center);
            }
            if input.key_pressed(Key::Minus) {
                cam.zoom_by(0.8, center);
            }
        }
        if self.edge_scroll > 0.0 && input.focused && input.hovering && pointer_in_view && input.touch_count() == 0 {
            let dpr = cam.dpr.max(1.0);
            let band = self.edge_scroll * dpr;
            let v = cam.viewport;
            let m = input.mouse;
            let (left, right) = (m.x - v.x, v.x + v.w - 1.0 - m.x);
            let (top, bottom) = (m.y - v.y, v.y + v.h - 1.0 - m.y);
            let nearest = left.min(right).min(top).min(bottom);
            if !over_ui || nearest <= EDGE_OVER_UI * dpr {
                // Depth into the band: 0 at its inner border, 1 at the edge; squared so the pan
                // starts gently and gets fast right at the edge. Axes are independent, so
                // corners scroll diagonally.
                let push = |dist: f32| {
                    let k = ((band - dist) / band).clamp(0.0, 1.0);
                    k * k
                };
                let d = Vec2::new(push(right) - push(left), push(bottom) - push(top));
                if d != Vec2::ZERO {
                    cam.pan_screen(-d * self.edge_speed * dpr * dt);
                }
            }
        }
        cam.update(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;
    use glam::Vec3;

    fn setup() -> (Camera, Input, CameraController) {
        let mut cam = Camera::new(Vec3::ZERO, 40.0);
        cam.viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        (cam, Input::new(), CameraController::default())
    }

    #[test]
    fn edge_of_viewport_pans() {
        let (mut cam, mut input, mut c) = setup();
        input.on_mouse_move(Vec2::new(799.0, 300.0));
        c.update(&mut cam, &input, 0.1, false);
        assert!(cam.target.x > 0.5, "panned right: {}", cam.target);
        let before = cam.target;
        input.on_mouse_move(Vec2::new(400.0, 1.0));
        c.update(&mut cam, &input, 0.1, false);
        assert!(cam.target.z < before.z - 0.5, "panned up (north): {}", cam.target);
    }

    #[test]
    fn closer_to_the_edge_is_faster() {
        let speed_at = |x: f32| {
            let (mut cam, mut input, mut c) = setup();
            input.on_mouse_move(Vec2::new(x, 300.0));
            c.update(&mut cam, &input, 0.1, false);
            cam.target.x
        };
        let (deep, mid, shallow, outside) = (speed_at(799.0), speed_at(780.0), speed_at(765.0), speed_at(700.0));
        assert!(deep > mid && mid > shallow && shallow > 0.0, "{deep} > {mid} > {shallow} > 0");
        assert_eq!(outside, 0.0, "outside the band");
    }

    #[test]
    fn no_edge_pan_in_the_middle_over_ui_or_outside() {
        let (mut cam, mut input, mut c) = setup();
        input.on_mouse_move(Vec2::new(400.0, 300.0));
        c.update(&mut cam, &input, 0.1, false);
        assert_eq!(cam.target, Vec3::ZERO);
        // Over a UI panel inside the band: no pan...
        input.on_mouse_move(Vec2::new(780.0, 300.0));
        c.update(&mut cam, &input, 0.1, true);
        assert_eq!(cam.target, Vec3::ZERO, "over UI");
        // ...but pressed against the very edge it pans even over UI.
        input.on_mouse_move(Vec2::new(799.0, 300.0));
        c.update(&mut cam, &input, 0.1, true);
        assert!(cam.target.x > 0.0, "edge beats UI");
        let before = cam.target;
        input.hovering = false;
        c.update(&mut cam, &input, 0.1, false);
        assert_eq!(cam.target, before, "pointer left the window");
    }

    #[test]
    fn losing_focus_stops_edge_scrolling() {
        let (mut cam, mut input, mut c) = setup();
        input.on_mouse_move(Vec2::new(799.0, 300.0));
        input.on_focus(false);
        // The pointer is still reported at the edge (no "cursor left" when switching apps).
        input.on_mouse_move(Vec2::new(799.0, 300.0));
        c.update(&mut cam, &input, 0.5, false);
        assert_eq!(cam.target, Vec3::ZERO, "no pan while unfocused");
        input.on_focus(true);
        c.update(&mut cam, &input, 0.1, false);
        assert!(cam.target.x > 0.0, "pans again once focused");
    }
}
