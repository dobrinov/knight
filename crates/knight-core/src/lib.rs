//! # knight-core
//!
//! The renderer-independent heart of Knight Engine. Everything here is plain Rust with no GPU or
//! windowing code, so it runs in tests, on servers and on every platform the runtime supports.
//!
//! - [`HexWorld`]: hex terrain with heights, depressions, water, materials and fog of war; cached
//!   chunk meshes; elevation-aware picking; saved and loaded as bytes (`to_bytes` / `load_bytes`).
//! - [`Camera`] / [`CameraController`]: fixed-orientation isometric camera with pan and zoom.
//! - [`Frame`]: per-frame drawing of worlds (sprites, overlays, labels) and immediate-mode UI.
//! - [`Scene`] / [`SceneStack`]: multiple UI levels (map → town → battle) with transitions.
//! - [`HexMover`] / [`Reservations`]: the only way to move — hex to hex, with terrain speed and
//!   real-time traffic rules.
//! - [`DayClock`] / [`ActionBudget`]: hybrid turns — timed days with a per-day action budget,
//!   everyone acting at once, no "End turn".
//! - [`ShrinkingZone`] / [`Wildfire`]: closing-in hazards (battle royale zones, spreading fire).
//! - [`Particles`]: bursts of sparks, smoke and fire.
//! - [`Speech`]: chat bubbles and emotes over characters, readable even in crowds.
//! - [`Audio`] / [`Mixer`] / [`synth`]: sound effects and music (WAV or procedural).
//! - [`Input`]: mouse, keyboard, touch, gestures and gamepads (with a virtual cursor so every
//!   game is controller-playable).
//! - [`Assets`]: texture atlas, images, animations and the built-in pixel font.

pub mod assets;
pub mod audio;
pub mod camera;
pub mod clock;
pub mod color;
pub mod controller;
pub mod frame;
pub mod fx;
pub mod geom;
pub mod image;
pub mod input;
pub mod mesh;
pub mod minimap;
pub mod nav;
pub mod save;
pub mod scene;
pub mod speech;
pub mod synth;
pub mod world;
pub mod zone;

pub use assets::{Animation, Assets, ImageId, Region};
pub use audio::{Audio, AudioCmd, Mixer, Sound, SoundId};
pub use camera::Camera;
pub use clock::{ActionBudget, DayClock};
pub use color::Color;
pub use controller::CameraController;
pub use frame::{Frame, FrameData, Sprite, Theme, WorldDraw, WorldPass};
pub use fx::{Burst, Particles};
pub use geom::{Ray, Rect};
pub use image::Image;
pub use input::{Gamepad, Input, Key, MouseButton, PadButton, PadState};
pub use minimap::Minimap;
pub use nav::{HexMover, NavError, Reservations, Step};
pub use save::SaveError;
pub use scene::{Context, RenderMode, RenderSettings, Scene, SceneStack, Stats, Time, Transition};
pub use speech::{ChatLine, Priority, Speech, SpeechConfig};
pub use synth::{Sfx, Wave};
pub use world::{ChunkKey, HexWorld, Material, MaterialId, Pick, Tile, Visibility, Water};
pub use zone::{ShrinkingZone, Wildfire, ZoneEvent, ZonePhase};

pub use glam;
pub use knight_hex as hex;
