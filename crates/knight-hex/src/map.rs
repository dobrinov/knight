//! Hex-keyed storage and map shapes.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

use crate::{Hex, Offset, Parity};

/// A fast hasher for packed hex keys (Fx-style multiply). Not DoS resistant; don't hash
/// untrusted input with it.
#[derive(Default, Clone, Copy)]
pub struct HexHasher(u64);

impl Hasher for HexHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
    }

    fn write_i32(&mut self, i: i32) {
        self.0 = (self.0.rotate_left(5) ^ i as u32 as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn write_u32(&mut self, i: u32) {
        self.write_i32(i as i32);
    }
}

pub type HexBuildHasher = BuildHasherDefault<HexHasher>;
pub type HexSet = HashSet<Hex, HexBuildHasher>;

/// A sparse map from hexes to values. Use it for tiles, units, fog, costs and so on.
#[derive(Clone, Debug)]
pub struct HexMap<T> {
    inner: HashMap<Hex, T, HexBuildHasher>,
}

impl<T> Default for HexMap<T> {
    fn default() -> Self {
        HexMap { inner: HashMap::default() }
    }
}

impl<T> HexMap<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(n: usize) -> Self {
        HexMap { inner: HashMap::with_capacity_and_hasher(n, Default::default()) }
    }

    /// Fill every hex of `shape` with `f(hex)`.
    pub fn from_shape(shape: impl IntoIterator<Item = Hex>, mut f: impl FnMut(Hex) -> T) -> Self {
        let mut m = HexMap::new();
        for h in shape {
            m.insert(h, f(h));
        }
        m
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn get(&self, h: Hex) -> Option<&T> {
        self.inner.get(&h)
    }

    pub fn get_mut(&mut self, h: Hex) -> Option<&mut T> {
        self.inner.get_mut(&h)
    }

    pub fn insert(&mut self, h: Hex, v: T) -> Option<T> {
        self.inner.insert(h, v)
    }

    pub fn remove(&mut self, h: Hex) -> Option<T> {
        self.inner.remove(&h)
    }

    pub fn contains(&self, h: Hex) -> bool {
        self.inner.contains_key(&h)
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }

    pub fn entry(&mut self, h: Hex) -> std::collections::hash_map::Entry<'_, Hex, T> {
        self.inner.entry(h)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Hex, &T)> {
        self.inner.iter().map(|(h, v)| (*h, v))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Hex, &mut T)> {
        self.inner.iter_mut().map(|(h, v)| (*h, v))
    }

    pub fn hexes(&self) -> impl Iterator<Item = Hex> + '_ {
        self.inner.keys().copied()
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.inner.values()
    }

    /// Neighbours of `h` that are present in the map.
    pub fn neighbors_of(&self, h: Hex) -> impl Iterator<Item = (Hex, &T)> {
        h.neighbors().into_iter().filter_map(|n| self.inner.get(&n).map(|v| (n, v)))
    }

    pub fn retain(&mut self, mut f: impl FnMut(Hex, &mut T) -> bool) {
        self.inner.retain(|h, v| f(*h, v))
    }
}

impl<T> std::ops::Index<Hex> for HexMap<T> {
    type Output = T;
    fn index(&self, h: Hex) -> &T {
        &self.inner[&h]
    }
}

impl<T> FromIterator<(Hex, T)> for HexMap<T> {
    fn from_iter<I: IntoIterator<Item = (Hex, T)>>(iter: I) -> Self {
        HexMap { inner: iter.into_iter().collect() }
    }
}

impl<T> IntoIterator for HexMap<T> {
    type Item = (Hex, T);
    type IntoIter = std::collections::hash_map::IntoIter<Hex, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

/// Common map shapes. Each returns the hexes in a deterministic order.
pub mod shapes {
    use super::*;

    /// A big hexagon of the given radius around `center`.
    pub fn hexagon(center: Hex, radius: i32) -> Vec<Hex> {
        center.range(radius).collect()
    }

    /// A `width × height` rectangle using offset rows (pointy hexes) or offset columns (flat).
    pub fn rectangle(width: i32, height: i32, pointy: bool, parity: Parity) -> Vec<Hex> {
        let mut out = Vec::with_capacity((width * height).max(0) as usize);
        for row in 0..height {
            for col in 0..width {
                let o = Offset::new(col, row);
                out.push(if pointy { o.to_hex_r(parity) } else { o.to_hex_q(parity) });
            }
        }
        out
    }

    /// An axial-aligned parallelogram (rhombus).
    pub fn parallelogram(q0: i32, q1: i32, r0: i32, r1: i32) -> Vec<Hex> {
        (q0..=q1).flat_map(|q| (r0..=r1).map(move |r| Hex::new(q, r))).collect()
    }

    /// A triangle with `size` hexes on each side.
    pub fn triangle(size: i32) -> Vec<Hex> {
        (0..=size).flat_map(|q| (0..=size - q).map(move |r| Hex::new(q, r))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_sizes() {
        assert_eq!(shapes::hexagon(Hex::ORIGIN, 2).len(), 19);
        assert_eq!(shapes::rectangle(5, 4, true, Parity::Odd).len(), 20);
        assert_eq!(shapes::parallelogram(0, 2, 0, 3).len(), 12);
        assert_eq!(shapes::triangle(3).len(), 10);
        let rect: HexSet = shapes::rectangle(7, 7, false, Parity::Even).into_iter().collect();
        assert_eq!(rect.len(), 49);
    }

    #[test]
    fn map_basics() {
        let mut m = HexMap::from_shape(shapes::hexagon(Hex::ORIGIN, 1), |h| h.length());
        assert_eq!(m.len(), 7);
        assert_eq!(m[Hex::ORIGIN], 0);
        assert_eq!(m.neighbors_of(Hex::ORIGIN).count(), 6);
        assert_eq!(m.neighbors_of(Hex::new(1, 0)).count(), 3);
        *m.get_mut(Hex::ORIGIN).unwrap() = 9;
        assert_eq!(m.get(Hex::ORIGIN), Some(&9));
        m.retain(|h, _| h != Hex::ORIGIN);
        assert!(!m.contains(Hex::ORIGIN));
    }
}
