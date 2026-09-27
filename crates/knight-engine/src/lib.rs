//! # Knight Engine
//!
//! A hex-based, isometric 2.5D game engine for strategy and RPG games, built on wgpu and winit so
//! the same game runs natively (Windows, macOS, Linux, iOS, Android) and on the web (WebGPU with
//! a WebGL2 fallback).
//!
//! This crate is the runtime (window, GPU renderer, audio output, gamepads, app loop). It
//! re-exports everything from [`knight_core`] (worlds, cameras, scenes, UI, input) and
//! [`knight_hex`] (grid math, as `hex`) so a game only depends on `knight-engine`. Networking
//! (`net`) comes with the default `net` feature.
//!
//! ```no_run
//! use knight_engine::*;
//!
//! struct Hello { world: HexWorld, camera: Camera }
//!
//! impl Scene for Hello {
//!     fn update(&mut self, ctx: &mut Context) -> Transition {
//!         self.camera.viewport = ctx.screen_rect();
//!         CameraController::default().update(&mut self.camera, &ctx.input, ctx.time.dt, false);
//!         Transition::None
//!     }
//!     fn draw(&mut self, f: &mut Frame) {
//!         f.world(&mut self.world, &self.camera);
//!         f.text(glam::Vec2::new(12.0, 12.0), "Hello, hexes!", 16.0, Color::WHITE);
//!     }
//! }
//!
//! fn main() {
//!     run(AppConfig::default(), |_ctx| {
//!         let mut world = HexWorld::new(hex::Layout::pointy(1.0));
//!         for h in hex::shapes::hexagon(hex::Hex::ORIGIN, 8) {
//!             world.set_tile(h, Tile::new(0, 0));
//!         }
//!         Box::new(Hello { world, camera: Camera::new(glam::Vec3::ZERO, 48.0) })
//!     });
//! }
//! ```

mod app;
mod audio_out;
mod gamepad_in;
mod renderer;

pub use app::{AppConfig, run, web_host};
pub use knight_core::*;
#[cfg(feature = "net")]
pub use knight_net as net;
pub use log;
pub use renderer::Renderer;
pub use web_time;
