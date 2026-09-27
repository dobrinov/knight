//! Gamepad input: gilrs natively, the W3C Gamepad API on the web. Reports raw pad state into
//! [`knight_core::Input`] every frame.

use knight_core::Input;

#[cfg(not(target_arch = "wasm32"))]
pub use native::GamepadIn;
#[cfg(target_arch = "wasm32")]
pub use web::GamepadIn;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use gilrs::{Axis, Button, Gilrs};
    use knight_core::glam::Vec2;
    use knight_core::{PadButton, PadState};

    pub struct GamepadIn {
        gilrs: Option<Gilrs>,
        slots: usize,
    }

    impl GamepadIn {
        pub fn new() -> Self {
            let gilrs = Gilrs::new().map_err(|e| log::warn!("knight gamepad: {e}")).ok();
            GamepadIn { gilrs, slots: 0 }
        }

        pub fn poll(&mut self, input: &mut Input) {
            let Some(g) = &mut self.gilrs else { return };
            while g.next_event().is_some() {}
            let map = [
                (PadButton::South, Button::South),
                (PadButton::East, Button::East),
                (PadButton::West, Button::West),
                (PadButton::North, Button::North),
                (PadButton::LeftBumper, Button::LeftTrigger),
                (PadButton::RightBumper, Button::RightTrigger),
                (PadButton::LeftTrigger, Button::LeftTrigger2),
                (PadButton::RightTrigger, Button::RightTrigger2),
                (PadButton::Select, Button::Select),
                (PadButton::Start, Button::Start),
                (PadButton::LeftStick, Button::LeftThumb),
                (PadButton::RightStick, Button::RightThumb),
                (PadButton::DPadUp, Button::DPadUp),
                (PadButton::DPadDown, Button::DPadDown),
                (PadButton::DPadLeft, Button::DPadLeft),
                (PadButton::DPadRight, Button::DPadRight),
                (PadButton::Home, Button::Mode),
            ];
            let mut slot = 0;
            for (_, pad) in g.gamepads() {
                let mut s = PadState { name: pad.name().to_string(), ..Default::default() };
                for (ours, theirs) in map {
                    s.buttons[ours as usize] = pad.is_pressed(theirs);
                }
                // gilrs sticks are y-up; ours are y-down like the screen.
                s.left = Vec2::new(pad.value(Axis::LeftStickX), -pad.value(Axis::LeftStickY));
                s.right = Vec2::new(pad.value(Axis::RightStickX), -pad.value(Axis::RightStickY));
                s.left_trigger = pad.button_data(Button::LeftTrigger2).map_or(0.0, |d| d.value());
                s.right_trigger = pad.button_data(Button::RightTrigger2).map_or(0.0, |d| d.value());
                input.set_gamepad(slot, Some(s));
                slot += 1;
            }
            for k in slot..self.slots {
                input.set_gamepad(k, None);
            }
            self.slots = slot;
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use knight_core::glam::Vec2;
    use knight_core::{PadButton, PadState};
    use wasm_bindgen::JsCast;

    pub struct GamepadIn {
        slots: usize,
    }

    impl GamepadIn {
        pub fn new() -> Self {
            GamepadIn { slots: 0 }
        }

        pub fn poll(&mut self, input: &mut Input) {
            let Some(nav) = web_sys::window().map(|w| w.navigator()) else { return };
            let Ok(list) = nav.get_gamepads() else { return };
            let mut slot = 0;
            for i in 0..list.length() {
                // Unchecked casts: real pads and polyfills / test doubles both work.
                let v = list.get(i);
                if !v.is_object() {
                    continue;
                }
                let pad: web_sys::Gamepad = v.unchecked_into();
                if !pad.connected() {
                    continue;
                }
                let mut s = PadState { name: pad.id(), ..Default::default() };
                // The "standard" mapping uses the same button order as PadButton::ALL.
                let buttons = pad.buttons();
                for (k, b) in PadButton::ALL.iter().enumerate() {
                    let v = buttons.get(k as u32);
                    if v.is_object() {
                        let btn: web_sys::GamepadButton = v.unchecked_into();
                        s.buttons[*b as usize] = btn.pressed();
                        match b {
                            PadButton::LeftTrigger => s.left_trigger = btn.value() as f32,
                            PadButton::RightTrigger => s.right_trigger = btn.value() as f32,
                            _ => {}
                        }
                    }
                }
                let axes = pad.axes();
                let ax = |k: u32| axes.get(k).as_f64().unwrap_or(0.0) as f32;
                s.left = Vec2::new(ax(0), ax(1));
                s.right = Vec2::new(ax(2), ax(3));
                input.set_gamepad(slot, Some(s));
                slot += 1;
            }
            for k in slot..self.slots {
                input.set_gamepad(k, None);
            }
            self.slots = slot;
        }
    }
}
