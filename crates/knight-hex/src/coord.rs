//! Axial / cube hex coordinates.
//!
//! Follows <https://www.redblobgames.com/grids/hexagons/>: a hex is stored as axial `(q, r)` and the
//! third cube coordinate is derived as `s = -q - r`.

use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// An integer hex coordinate in axial form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hex {
    pub q: i32,
    pub r: i32,
}

/// Neighbour offsets, indexed by direction 0..6 (counter-clockwise starting at "east" for pointy
/// layouts). Direction `i` is shared by the edge between corners `i` and `i + 1` in
/// [`crate::Layout::edge_direction`].
pub const DIRECTIONS: [Hex; 6] =
    [Hex::new(1, 0), Hex::new(1, -1), Hex::new(0, -1), Hex::new(-1, 0), Hex::new(-1, 1), Hex::new(0, 1)];

/// Diagonal offsets (distance 2, between two neighbours).
pub const DIAGONALS: [Hex; 6] =
    [Hex::new(2, -1), Hex::new(1, -2), Hex::new(-1, -1), Hex::new(-2, 1), Hex::new(-1, 2), Hex::new(1, 1)];

impl Hex {
    pub const ORIGIN: Hex = Hex::new(0, 0);

    pub const fn new(q: i32, r: i32) -> Self {
        Hex { q, r }
    }

    /// Build from cube coordinates. Panics in debug builds if `q + r + s != 0`.
    pub fn from_cube(q: i32, r: i32, s: i32) -> Self {
        debug_assert_eq!(q + r + s, 0, "cube coordinates must sum to zero");
        Hex { q, r }
    }

    pub const fn s(self) -> i32 {
        -self.q - self.r
    }

    pub const fn cube(self) -> [i32; 3] {
        [self.q, self.r, self.s()]
    }

    /// Distance from the origin, in steps.
    pub const fn length(self) -> i32 {
        (self.q.abs() + self.r.abs() + self.s().abs()) / 2
    }

    pub const fn distance(self, other: Hex) -> i32 {
        Hex::new(self.q - other.q, self.r - other.r).length()
    }

    pub const fn direction(dir: usize) -> Hex {
        DIRECTIONS[dir % 6]
    }

    pub fn neighbor(self, dir: usize) -> Hex {
        self + Hex::direction(dir)
    }

    pub fn neighbors(self) -> [Hex; 6] {
        DIRECTIONS.map(|d| self + d)
    }

    pub fn diagonal_neighbor(self, dir: usize) -> Hex {
        self + DIAGONALS[dir % 6]
    }

    /// Index of the direction pointing from `self` to an adjacent `other`.
    pub fn direction_to(self, other: Hex) -> Option<usize> {
        let d = other - self;
        DIRECTIONS.iter().position(|&x| x == d)
    }

    /// Rotate 60° counter-clockwise around the origin: `[q, r, s] -> [-s, -q, -r]`.
    pub const fn rotate_left(self) -> Hex {
        Hex::new(-self.s(), -self.q)
    }

    /// Rotate 60° clockwise around the origin: `[q, r, s] -> [-r, -s, -q]`.
    pub const fn rotate_right(self) -> Hex {
        Hex::new(-self.r, -self.s())
    }

    /// Rotate by `steps` × 60° around `center` (positive = counter-clockwise).
    pub fn rotate_around(self, center: Hex, steps: i32) -> Hex {
        let mut v = self - center;
        for _ in 0..steps.rem_euclid(6) {
            v = v.rotate_left();
        }
        center + v
    }

    pub const fn reflect_q(self) -> Hex {
        Hex::new(self.q, self.s())
    }

    pub const fn reflect_r(self) -> Hex {
        Hex::new(self.s(), self.r)
    }

    pub const fn reflect_s(self) -> Hex {
        Hex::new(self.r, self.q)
    }

    /// Hexes on the straight line from `self` to `other`, both ends included.
    pub fn line_to(self, other: Hex) -> Vec<Hex> {
        let n = self.distance(other);
        // Nudge both ends so that lines on exact edges resolve consistently.
        let a = FracHex::new(self.q as f64 + 1e-6, self.r as f64 + 1e-6);
        let b = FracHex::new(other.q as f64 + 1e-6, other.r as f64 + 1e-6);
        let steps = n.max(1) as f64;
        (0..=n).map(|i| a.lerp(b, i as f64 / steps).round()).collect()
    }

    /// Hexes at exactly `radius` steps from `self`, walking counter-clockwise.
    pub fn ring(self, radius: i32) -> Vec<Hex> {
        if radius <= 0 {
            return vec![self];
        }
        let mut out = Vec::with_capacity(6 * radius as usize);
        let mut h = self + Hex::direction(4) * radius;
        for side in 0..6 {
            for _ in 0..radius {
                out.push(h);
                h = h.neighbor(side);
            }
        }
        out
    }

    /// Hexes within `radius`, ordered from the centre outwards ring by ring.
    pub fn spiral(self, radius: i32) -> Vec<Hex> {
        let mut out = vec![self];
        for k in 1..=radius {
            out.extend(self.ring(k));
        }
        out
    }

    /// Hexes within `radius` in axial scan order.
    pub fn range(self, radius: i32) -> impl Iterator<Item = Hex> {
        (-radius..=radius).flat_map(move |dq| {
            let r0 = (-radius).max(-dq - radius);
            let r1 = radius.min(-dq + radius);
            (r0..=r1).map(move |dr| Hex::new(self.q + dq, self.r + dr))
        })
    }

    /// Pack into a single `u32` (both coordinates must fit in `i16`).
    pub const fn pack(self) -> u32 {
        (((self.q + 32768) as u32) << 16) | ((self.r + 32768) as u32 & 0xffff)
    }

    pub const fn unpack(key: u32) -> Hex {
        Hex::new((key >> 16) as i32 - 32768, (key & 0xffff) as i32 - 32768)
    }
}

impl Add for Hex {
    type Output = Hex;
    fn add(self, o: Hex) -> Hex {
        Hex::new(self.q + o.q, self.r + o.r)
    }
}

impl Sub for Hex {
    type Output = Hex;
    fn sub(self, o: Hex) -> Hex {
        Hex::new(self.q - o.q, self.r - o.r)
    }
}

impl AddAssign for Hex {
    fn add_assign(&mut self, o: Hex) {
        *self = *self + o;
    }
}

impl SubAssign for Hex {
    fn sub_assign(&mut self, o: Hex) {
        *self = *self - o;
    }
}

impl Mul<i32> for Hex {
    type Output = Hex;
    fn mul(self, k: i32) -> Hex {
        Hex::new(self.q * k, self.r * k)
    }
}

impl Neg for Hex {
    type Output = Hex;
    fn neg(self) -> Hex {
        Hex::new(-self.q, -self.r)
    }
}

impl std::fmt::Display for Hex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.q, self.r)
    }
}

/// A fractional hex coordinate, produced by pixel conversion and interpolation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FracHex {
    pub q: f64,
    pub r: f64,
}

impl FracHex {
    pub const fn new(q: f64, r: f64) -> Self {
        FracHex { q, r }
    }

    pub fn s(self) -> f64 {
        -self.q - self.r
    }

    /// Round to the nearest hex, fixing up the component with the largest rounding error.
    pub fn round(self) -> Hex {
        let s = self.s();
        let mut q = self.q.round();
        let mut r = self.r.round();
        let rs = s.round();
        let dq = (q - self.q).abs();
        let dr = (r - self.r).abs();
        let ds = (rs - s).abs();
        if dq > dr && dq > ds {
            q = -r - rs;
        } else if dr > ds {
            r = -q - rs;
        }
        Hex::new(q as i32, r as i32)
    }

    pub fn lerp(self, other: FracHex, t: f64) -> FracHex {
        FracHex::new(self.q + (other.q - self.q) * t, self.r + (other.r - self.r) * t)
    }
}

impl From<Hex> for FracHex {
    fn from(h: Hex) -> Self {
        FracHex::new(h.q as f64, h.r as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_distance() {
        let a = Hex::new(1, -3);
        let b = Hex::new(3, -7);
        assert_eq!(a + b, Hex::new(4, -10));
        assert_eq!(b - a, Hex::new(2, -4));
        assert_eq!(a * 2, Hex::new(2, -6));
        assert_eq!(Hex::new(3, -7).distance(Hex::ORIGIN), 7);
        assert_eq!(a.s(), 2);
    }

    #[test]
    fn neighbors_are_distance_one() {
        for d in 0..6 {
            assert_eq!(Hex::ORIGIN.neighbor(d).length(), 1);
            assert_eq!(Hex::ORIGIN.diagonal_neighbor(d).length(), 2);
            assert_eq!(Hex::ORIGIN.direction_to(Hex::direction(d)), Some(d));
        }
    }

    #[test]
    fn rotation() {
        let a = Hex::new(1, -3);
        assert_eq!(a.rotate_right(), Hex::new(3, -2));
        assert_eq!(a.rotate_left(), Hex::new(-2, -1));
        assert_eq!(a.rotate_left().rotate_right(), a);
        let mut h = a;
        for _ in 0..6 {
            h = h.rotate_left();
        }
        assert_eq!(h, a);
        assert_eq!(Hex::new(2, 0).rotate_around(Hex::new(1, 0), 6), Hex::new(2, 0));
        assert_eq!(Hex::direction(0).rotate_left(), Hex::direction(1));
    }

    #[test]
    fn reflection() {
        let h = Hex::new(1, 2);
        assert_eq!(h.reflect_q().reflect_q(), h);
        assert_eq!(h.reflect_s(), Hex::new(2, 1));
    }

    #[test]
    fn rounding() {
        let a = FracHex::new(0.0, 0.0);
        let b = FracHex::new(1.0, -1.0);
        let c = FracHex::new(0.0, -1.0);
        let r = |f: FracHex| f.round();
        assert_eq!(r(FracHex::new(0.0, 0.0).lerp(FracHex::new(10.0, -20.0), 0.5)), Hex::new(5, -10));
        assert_eq!(r(a.lerp(b, 0.499)), Hex::new(0, 0));
        assert_eq!(r(a.lerp(b, 0.501)), Hex::new(1, -1));
        let third = FracHex::new(a.q * 0.4 + b.q * 0.3 + c.q * 0.3, a.r * 0.4 + b.r * 0.3 + c.r * 0.3);
        assert_eq!(r(third), Hex::new(0, 0));
    }

    #[test]
    fn line() {
        assert_eq!(
            Hex::new(0, 0).line_to(Hex::new(1, -5)),
            vec![Hex::new(0, 0), Hex::new(0, -1), Hex::new(0, -2), Hex::new(1, -3), Hex::new(1, -4), Hex::new(1, -5)]
        );
        assert_eq!(Hex::new(2, 2).line_to(Hex::new(2, 2)), vec![Hex::new(2, 2)]);
    }

    #[test]
    fn ring_spiral_range() {
        assert_eq!(Hex::ORIGIN.ring(0), vec![Hex::ORIGIN]);
        for k in 1..6 {
            let ring = Hex::new(3, -1).ring(k);
            assert_eq!(ring.len(), 6 * k as usize);
            assert!(ring.iter().all(|h| h.distance(Hex::new(3, -1)) == k));
        }
        assert_eq!(Hex::ORIGIN.spiral(3).len(), 37);
        assert_eq!(Hex::ORIGIN.range(3).count(), 37);
        assert!(Hex::new(1, 1).range(2).all(|h| h.distance(Hex::new(1, 1)) <= 2));
    }

    #[test]
    fn packing() {
        for h in [Hex::new(0, 0), Hex::new(-5, 17), Hex::new(32000, -32000)] {
            assert_eq!(Hex::unpack(h.pack()), h);
        }
    }
}
