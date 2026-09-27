//! GPU-ready vertex formats and CPU mesh buffers.

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};

use crate::{Color, Region};

/// A world-space vertex.
///
/// `depth` overrides the clip-space depth when it is `>= 0`. Billboards use it so the whole
/// sprite sorts at the depth of its feet, which makes upright sprites occlude like painter's
/// algorithm while terrain still uses the depth buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    pub depth: f32,
}

impl Vertex {
    pub fn new(pos: Vec3, uv: [f32; 2], color: [u8; 4]) -> Self {
        Vertex { pos: pos.to_array(), uv, color, depth: -1.0 }
    }
}

/// A screen-space (UI) vertex in physical pixels.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

/// Indexed triangle list.
#[derive(Clone, Debug, Default)]
pub struct Mesh<V> {
    pub vertices: Vec<V>,
    pub indices: Vec<u32>,
}

impl<V: Copy> Mesh<V> {
    pub fn new() -> Self {
        Mesh { vertices: Vec::new(), indices: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Append a quad given in order top-left, top-right, bottom-right, bottom-left.
    pub fn quad(&mut self, v: [V; 4]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&v);
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Append a triangle fan (first vertex is the hub, closed ring).
    pub fn fan(&mut self, hub: V, ring: &[V]) {
        let base = self.vertices.len() as u32;
        self.vertices.push(hub);
        self.vertices.extend_from_slice(ring);
        let n = ring.len() as u32;
        for i in 0..n {
            self.indices.extend_from_slice(&[base, base + 1 + i, base + 1 + (i + 1) % n]);
        }
    }

    pub fn append(&mut self, other: &Mesh<V>) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }
}

pub type WorldMesh = Mesh<Vertex>;
pub type UiMesh = Mesh<UiVertex>;

impl UiMesh {
    /// A textured, tinted rectangle in physical pixels.
    pub fn rect(&mut self, min: Vec2, max: Vec2, region: &Region, color: Color) {
        self.rect_uv(min, max, region.uv0, region.uv1, color);
    }

    pub fn rect_uv(&mut self, min: Vec2, max: Vec2, uv0: [f32; 2], uv1: [f32; 2], color: Color) {
        let c = color.to_rgba8();
        self.quad([
            UiVertex { pos: [min.x, min.y], uv: uv0, color: c },
            UiVertex { pos: [max.x, min.y], uv: [uv1[0], uv0[1]], color: c },
            UiVertex { pos: [max.x, max.y], uv: uv1, color: c },
            UiVertex { pos: [min.x, max.y], uv: [uv0[0], uv1[1]], color: c },
        ]);
    }
}
