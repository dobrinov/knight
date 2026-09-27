//! # knight-hex
//!
//! Hexagon grid math for Knight Engine, following Red Blob Games' guide
//! (<https://www.redblobgames.com/grids/hexagons/>). It has no dependencies and no rendering, so
//! game rules, servers and tools can use it directly.
//!
//! - [`Hex`] / [`FracHex`]: axial coordinates, neighbours, distance, rotation, lines, rings.
//! - [`Layout`]: hex ↔ 2D ground coordinates for pointy or flat hexes.
//! - [`Offset`] / [`Doubled`]: alternative coordinate systems for rectangular storage.
//! - [`HexMap`] and [`shapes`]: sparse per-hex storage and common map shapes.
//! - [`path`]: A*, budgeted movement ranges and flow fields.
//! - [`vision`]: height-aware line of sight and field of view.
//! - [`noise`]: deterministic RNG and value noise for procedural maps.

mod coord;
mod layout;
mod map;
pub mod noise;
mod offset;
pub mod path;
pub mod vision;

pub use coord::{DIAGONALS, DIRECTIONS, FracHex, Hex};
pub use layout::{Layout, Orientation, Point};
pub use map::{HexBuildHasher, HexHasher, HexMap, HexSet, shapes};
pub use noise::{Rng, ValueNoise};
pub use offset::{Doubled, Offset, Parity};
pub use path::{FlowField, Path, Reachable};
