//! Platform-independent input state: mouse, keyboard, touch and gestures.
//!
//! The runtime feeds raw events in; games read a per-frame snapshot. Positions are physical
//! pixels. A single touch acts as the left mouse button; two touches produce pinch and pan
//! gestures.

use std::collections::{HashMap, HashSet};

use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    const ALL: [MouseButton; 3] = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];
    fn index(self) -> usize {
        self as usize
    }
}

/// Keys games commonly need. Letters and digits are layout-independent physical keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Space,
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Shift,
    Control,
    Alt,
    Super,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Minus,
    Equal,
    Comma,
    Period,
    Slash,
    BracketLeft,
    BracketRight,
    Backquote,
}

impl Key {
    pub const DIGITS: [Key; 10] = [
        Key::Digit0,
        Key::Digit1,
        Key::Digit2,
        Key::Digit3,
        Key::Digit4,
        Key::Digit5,
        Key::Digit6,
        Key::Digit7,
        Key::Digit8,
        Key::Digit9,
    ];
}

/// Gamepad buttons, in the standard (Xbox-style) layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadButton {
    /// A / Cross.
    South,
    /// B / Circle.
    East,
    /// X / Square.
    West,
    /// Y / Triangle.
    North,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    Select,
    Start,
    LeftStick,
    RightStick,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Home,
}

impl PadButton {
    pub const COUNT: usize = 17;
    /// In W3C "standard" gamepad mapping order.
    pub const ALL: [PadButton; 17] = [
        PadButton::South,
        PadButton::East,
        PadButton::West,
        PadButton::North,
        PadButton::LeftBumper,
        PadButton::RightBumper,
        PadButton::LeftTrigger,
        PadButton::RightTrigger,
        PadButton::Select,
        PadButton::Start,
        PadButton::LeftStick,
        PadButton::RightStick,
        PadButton::DPadUp,
        PadButton::DPadDown,
        PadButton::DPadLeft,
        PadButton::DPadRight,
        PadButton::Home,
    ];
}

/// One gamepad's state this frame.
#[derive(Clone, Debug, Default)]
pub struct Gamepad {
    pub connected: bool,
    pub name: String,
    down: [bool; PadButton::COUNT],
    prev: [bool; PadButton::COUNT],
    /// Left stick, right stick (x right, y down), each in -1..1.
    pub left: Vec2,
    pub right: Vec2,
    /// Analog triggers 0..1.
    pub left_trigger: f32,
    pub right_trigger: f32,
}

impl Gamepad {
    pub fn down(&self, b: PadButton) -> bool {
        self.down[b as usize]
    }

    pub fn pressed(&self, b: PadButton) -> bool {
        self.down[b as usize] && !self.prev[b as usize]
    }

    pub fn released(&self, b: PadButton) -> bool {
        !self.down[b as usize] && self.prev[b as usize]
    }

    /// Any input at all (to switch prompts / show the pad cursor).
    pub fn active(&self) -> bool {
        self.down.iter().any(|d| *d) || self.left.length() > 0.3 || self.right.length() > 0.3
    }
}

/// Raw state a runtime backend reports for one pad.
#[derive(Clone, Debug, Default)]
pub struct PadState {
    pub name: String,
    pub buttons: [bool; PadButton::COUNT],
    pub left: Vec2,
    pub right: Vec2,
    pub left_trigger: f32,
    pub right_trigger: f32,
}

fn deadzone(v: Vec2) -> Vec2 {
    let l = v.length();
    if l < 0.18 { Vec2::ZERO } else { v / l * ((l - 0.18) / 0.82).min(1.0) }
}

#[derive(Clone, Copy, Debug, Default)]
struct ButtonState {
    down: bool,
    pressed: bool,
    released: bool,
    press_pos: Vec2,
    dragged: bool,
}

/// Movement (physical pixels) before a press becomes a drag instead of a click.
pub const DRAG_THRESHOLD: f32 = 6.0;

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub mouse: Vec2,
    pub mouse_delta: Vec2,
    /// Wheel movement this frame in "lines" (positive = away from the user / zoom in).
    pub wheel: f32,
    /// Pinch zoom factor this frame (1 = none). Includes trackpad pinch where available.
    pub pinch: f32,
    /// Two-finger pan this frame, physical pixels.
    pub two_finger_pan: Vec2,
    /// Text typed this frame.
    pub text: String,
    /// True while the pointer is over the window.
    pub hovering: bool,
    /// True while the window has keyboard focus. Edge scrolling and keyboard panning only work
    /// while focused, so switching apps (Cmd/Alt-Tab) never leaves the camera sliding.
    pub focused: bool,
    buttons: [ButtonState; 3],
    keys_down: HashSet<Key>,
    keys_pressed: HashSet<Key>,
    keys_released: HashSet<Key>,
    touches: HashMap<u64, Vec2>,
    touch_primary: Option<u64>,
    /// Filled by the runtime from the previous frame's UI (see [`crate::Frame::block`]).
    pub(crate) ui_rects: Vec<crate::Rect>,
    pub(crate) ui_clicked: HashSet<u64>,
    /// Connected gamepads (index = slot).
    pub gamepads: Vec<Gamepad>,
    /// Drive the mouse, keys and camera from the first gamepad (on by default), so any game
    /// works with a controller: left stick = cursor, A = click, B = right click, X = middle,
    /// Y = Space, Start = Enter, Select = Escape, D-pad = arrow keys, right stick = pan,
    /// triggers / bumpers = zoom.
    pub gamepad_cursor: bool,
    /// Seconds since the gamepad was last used (shows / hides the pad cursor).
    pub pad_idle: f32,
    pad_keys: HashSet<Key>,
}

impl Input {
    pub fn new() -> Self {
        Input {
            pinch: 1.0,
            hovering: false,
            focused: true,
            gamepad_cursor: true,
            pad_idle: f32::MAX,
            ..Default::default()
        }
    }

    // --- Queries ------------------------------------------------------------------------------

    pub fn down(&self, b: MouseButton) -> bool {
        self.buttons[b.index()].down
    }

    pub fn pressed(&self, b: MouseButton) -> bool {
        self.buttons[b.index()].pressed
    }

    pub fn released(&self, b: MouseButton) -> bool {
        self.buttons[b.index()].released
    }

    /// Released this frame without having been dragged.
    pub fn clicked(&self, b: MouseButton) -> bool {
        let s = &self.buttons[b.index()];
        s.released && !s.dragged
    }

    /// While a button is held and has moved past the drag threshold: (press position, now).
    pub fn drag(&self, b: MouseButton) -> Option<(Vec2, Vec2)> {
        let s = &self.buttons[b.index()];
        (s.down && s.dragged).then_some((s.press_pos, self.mouse))
    }

    /// A drag that ended this frame: (press position, release position).
    pub fn drag_released(&self, b: MouseButton) -> Option<(Vec2, Vec2)> {
        let s = &self.buttons[b.index()];
        (s.released && s.dragged).then_some((s.press_pos, self.mouse))
    }

    pub fn key_down(&self, k: Key) -> bool {
        self.keys_down.contains(&k)
    }

    pub fn key_pressed(&self, k: Key) -> bool {
        self.keys_pressed.contains(&k)
    }

    pub fn key_released(&self, k: Key) -> bool {
        self.keys_released.contains(&k)
    }

    pub fn shift(&self) -> bool {
        self.key_down(Key::Shift)
    }

    pub fn ctrl(&self) -> bool {
        self.key_down(Key::Control) || self.key_down(Key::Super)
    }

    pub fn touch_count(&self) -> usize {
        self.touches.len()
    }

    /// Was the pointer over UI drawn last frame (panels, buttons)? Use this to avoid clicking
    /// through UI into the world.
    pub fn pointer_over_ui(&self) -> bool {
        self.ui_rects.iter().any(|r| r.contains(self.mouse))
    }

    /// Was the UI button with this id clicked (during the previous frame's draw)?
    pub fn ui_clicked(&self, id: &str) -> bool {
        self.ui_clicked.contains(&crate::frame::ui_id(id))
    }

    /// Number key pressed this frame (0-9), if any.
    pub fn digit_pressed(&self) -> Option<usize> {
        Key::DIGITS.iter().position(|k| self.key_pressed(*k))
    }

    /// The first connected gamepad, if any.
    pub fn pad(&self) -> Option<&Gamepad> {
        self.gamepads.iter().find(|g| g.connected)
    }

    /// Is the gamepad the input device being used right now?
    pub fn using_gamepad(&self) -> bool {
        self.pad_idle < 4.0
    }

    // --- Feeding (called by the runtime) ------------------------------------------------------

    /// Report a gamepad's raw state for this frame (`None` = disconnected).
    pub fn set_gamepad(&mut self, slot: usize, state: Option<PadState>) {
        if self.gamepads.len() <= slot {
            self.gamepads.resize(slot + 1, Gamepad::default());
        }
        let g = &mut self.gamepads[slot];
        g.prev = g.down;
        match state {
            Some(s) => {
                g.connected = true;
                g.name = s.name;
                g.down = s.buttons;
                g.left = deadzone(s.left);
                g.right = deadzone(s.right);
                g.left_trigger = s.left_trigger;
                g.right_trigger = s.right_trigger;
            }
            None => *g = Gamepad::default(),
        }
    }

    /// Turn the first gamepad into mouse / keyboard / gesture input. Called by the runtime
    /// after `set_gamepad`, before the frame's update. `screen` is the window size.
    pub fn apply_gamepad(&mut self, dt: f32, screen: Vec2, scale: f32) {
        let Some(g) = self.gamepads.iter().find(|g| g.connected) else { return };
        // Copy the state out (without the name) so the pad can drive `self` below.
        let Gamepad { down, prev, left, right, left_trigger, right_trigger, .. } = *g;
        let pad = Gamepad { down, prev, left, right, left_trigger, right_trigger, ..Default::default() };
        if pad.active() || left_trigger > 0.1 || right_trigger > 0.1 {
            self.pad_idle = 0.0;
        } else {
            self.pad_idle = (self.pad_idle + dt).min(1e6);
        }
        if !self.gamepad_cursor {
            return;
        }
        if left != Vec2::ZERO {
            // Accelerating cursor: precise when nudged, fast when pushed.
            let speed = (350.0 + 1100.0 * left.length().powi(2)) * scale;
            let p = (self.mouse + left * speed * dt).clamp(Vec2::ZERO, screen);
            self.on_mouse_move(p);
        }
        for (b, m) in [
            (PadButton::South, MouseButton::Left),
            (PadButton::East, MouseButton::Right),
            (PadButton::West, MouseButton::Middle),
        ] {
            if pad.pressed(b) {
                self.on_button(m, true);
            }
            if pad.released(b) {
                self.on_button(m, false);
            }
        }
        let keys = [
            (PadButton::North, Key::Space),
            (PadButton::Start, Key::Enter),
            (PadButton::Select, Key::Escape),
            (PadButton::DPadUp, Key::Up),
            (PadButton::DPadDown, Key::Down),
            (PadButton::DPadLeft, Key::Left),
            (PadButton::DPadRight, Key::Right),
        ];
        for (b, k) in keys {
            if pad.pressed(b) {
                self.pad_keys.insert(k);
                self.on_key(k, true);
            }
            if pad.released(b) && self.pad_keys.remove(&k) {
                self.on_key(k, false);
            }
        }
        if right != Vec2::ZERO {
            self.two_finger_pan -= right * 900.0 * scale * dt;
        }
        let zoom = right_trigger - left_trigger + if pad.down(PadButton::RightBumper) { 1.0 } else { 0.0 }
            - if pad.down(PadButton::LeftBumper) { 1.0 } else { 0.0 };
        if zoom.abs() > 0.05 {
            self.pinch *= 1.0 + zoom * 1.5 * dt;
        }
    }

    pub fn on_mouse_move(&mut self, p: Vec2) {
        self.hovering = true;
        self.mouse_delta += p - self.mouse;
        self.mouse = p;
        for s in &mut self.buttons {
            if s.down && !s.dragged && p.distance(s.press_pos) > DRAG_THRESHOLD {
                s.dragged = true;
            }
        }
    }

    /// The window gained or lost focus. Losing it releases everything that is held.
    pub fn on_focus(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.reset();
            self.hovering = false;
        }
    }

    pub fn on_button(&mut self, b: MouseButton, down: bool) {
        if down {
            // Clicking into the window means it has focus again.
            self.focused = true;
        }
        let s = &mut self.buttons[b.index()];
        if down && !s.down {
            s.down = true;
            s.pressed = true;
            s.press_pos = self.mouse;
            s.dragged = false;
        } else if !down && s.down {
            s.down = false;
            s.released = true;
        }
    }

    pub fn on_wheel(&mut self, lines: f32) {
        self.wheel += lines;
    }

    pub fn on_pinch(&mut self, delta: f32) {
        self.pinch *= 1.0 + delta;
    }

    pub fn on_key(&mut self, k: Key, down: bool) {
        let modifier = matches!(k, Key::Shift | Key::Control | Key::Alt | Key::Super);
        let chord = self.key_down(Key::Super) || self.key_down(Key::Control);
        if down {
            if chord && !modifier {
                // A shortcut (Cmd+S, Ctrl+1...): report the press, but never treat the key as
                // held. macOS swallows key-ups while Cmd is down, which would leave it stuck.
                self.keys_pressed.insert(k);
                self.keys_released.insert(k);
            } else if self.keys_down.insert(k) {
                self.keys_pressed.insert(k);
            }
        } else if self.keys_down.remove(&k) {
            self.keys_released.insert(k);
            if k == Key::Super {
                // Key-ups of anything pressed during the chord may never arrive: release all.
                for other in self.keys_down.drain().collect::<Vec<_>>() {
                    self.keys_released.insert(other);
                }
            }
        }
    }

    pub fn on_text(&mut self, s: &str) {
        self.text.push_str(s);
    }

    /// Release everything (e.g. when the window loses focus).
    pub fn reset(&mut self) {
        for k in self.keys_down.drain() {
            self.keys_released.insert(k);
        }
        for b in MouseButton::ALL {
            if self.buttons[b.index()].down {
                self.on_button(b, false);
            }
        }
        self.touches.clear();
        self.touch_primary = None;
    }

    pub fn on_touch_start(&mut self, id: u64, p: Vec2) {
        self.touches.insert(id, p);
        match self.touches.len() {
            1 => {
                self.touch_primary = Some(id);
                self.on_mouse_move(p);
                self.mouse_delta = Vec2::ZERO;
                self.on_button(MouseButton::Left, true);
            }
            2
                // Second finger: this is a gesture, not a click or drag.
                if self.buttons[0].down => {
                    self.buttons[0].dragged = true;
                    self.on_button(MouseButton::Left, false);
                }
            _ => {}
        }
    }

    pub fn on_touch_move(&mut self, id: u64, p: Vec2) {
        let Some(old) = self.touches.get(&id).copied() else { return };
        if self.touches.len() == 2 {
            let other = self.touches.iter().find(|(k, _)| **k != id).map(|(_, v)| *v).unwrap();
            let (d0, d1) = (old - other, p - other);
            if d0.length() > 1.0 && d1.length() > 1.0 {
                self.pinch *= d1.length() / d0.length();
            }
            self.two_finger_pan += (p - old) * 0.5;
        }
        self.touches.insert(id, p);
        if self.touch_primary == Some(id) && self.touches.len() == 1 {
            self.on_mouse_move(p);
        }
    }

    pub fn on_touch_end(&mut self, id: u64) {
        self.touches.remove(&id);
        if self.touch_primary == Some(id) {
            self.touch_primary = None;
            if self.buttons[0].down {
                self.on_button(MouseButton::Left, false);
            }
        }
        if self.touches.is_empty() {
            self.touch_primary = None;
        }
    }

    /// Clear per-frame state. Called by the runtime after each frame.
    pub fn end_frame(&mut self) {
        for s in &mut self.buttons {
            s.pressed = false;
            s.released = false;
        }
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.mouse_delta = Vec2::ZERO;
        self.wheel = 0.0;
        self.pinch = 1.0;
        self.two_finger_pan = Vec2::ZERO;
        self.text.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_vs_drag() {
        let mut i = Input::new();
        i.on_mouse_move(Vec2::new(10.0, 10.0));
        i.on_button(MouseButton::Left, true);
        assert!(i.pressed(MouseButton::Left));
        i.end_frame();
        i.on_button(MouseButton::Left, false);
        assert!(i.clicked(MouseButton::Left));
        i.end_frame();

        i.on_button(MouseButton::Left, true);
        i.on_mouse_move(Vec2::new(40.0, 10.0));
        assert!(i.drag(MouseButton::Left).is_some());
        i.on_button(MouseButton::Left, false);
        assert!(!i.clicked(MouseButton::Left));
        assert!(i.drag_released(MouseButton::Left).is_some());
    }

    #[test]
    fn pinch_gesture() {
        let mut i = Input::new();
        i.on_touch_start(1, Vec2::new(100.0, 100.0));
        i.on_touch_start(2, Vec2::new(200.0, 100.0));
        assert!(!i.down(MouseButton::Left));
        i.on_touch_move(2, Vec2::new(300.0, 100.0));
        assert!((i.pinch - 2.0).abs() < 1e-4);
    }

    #[test]
    fn gamepad_drives_cursor_and_clicks() {
        let mut i = Input::new();
        i.on_mouse_move(Vec2::new(100.0, 100.0));
        let mut s = PadState { left: Vec2::new(1.0, 0.0), ..Default::default() };
        i.set_gamepad(0, Some(s.clone()));
        i.apply_gamepad(0.1, Vec2::new(1000.0, 800.0), 1.0);
        assert!(i.mouse.x > 200.0, "cursor moved: {}", i.mouse);
        assert!(i.using_gamepad());
        i.end_frame();
        s.left = Vec2::ZERO;
        s.buttons[PadButton::South as usize] = true;
        i.set_gamepad(0, Some(s.clone()));
        i.apply_gamepad(0.016, Vec2::new(1000.0, 800.0), 1.0);
        assert!(i.pressed(MouseButton::Left));
        i.end_frame();
        s.buttons[PadButton::South as usize] = false;
        s.buttons[PadButton::Start as usize] = true;
        i.set_gamepad(0, Some(s));
        i.apply_gamepad(0.016, Vec2::new(1000.0, 800.0), 1.0);
        assert!(i.clicked(MouseButton::Left));
        assert!(i.key_pressed(Key::Enter));
    }

    #[test]
    fn cmd_shortcuts_never_leave_keys_held() {
        // macOS: Cmd down, S down, S up is swallowed, Cmd up.
        let mut i = Input::new();
        i.on_key(Key::Super, true);
        i.on_key(Key::S, true);
        assert!(i.key_pressed(Key::S), "the shortcut press is still reported");
        assert!(!i.key_down(Key::S), "but S is not considered held");
        i.on_key(Key::Super, false);
        i.end_frame();
        assert!(!i.key_down(Key::S));
        // Holding S first, then Cmd, then releasing Cmd (S's key-up lost) releases S too.
        i.on_key(Key::S, true);
        i.on_key(Key::Super, true);
        i.on_key(Key::Super, false);
        assert!(!i.key_down(Key::S));
    }

    #[test]
    fn keys() {
        let mut i = Input::new();
        i.on_key(Key::Q, true);
        assert!(i.key_pressed(Key::Q) && i.key_down(Key::Q));
        i.end_frame();
        i.on_key(Key::Q, true);
        assert!(!i.key_pressed(Key::Q));
        i.on_key(Key::Q, false);
        assert!(i.key_released(Key::Q));
    }
}
