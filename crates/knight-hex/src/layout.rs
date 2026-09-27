//! Conversion between hexes and 2D "layout space" (the flat ground plane before any projection).

use crate::{FracHex, Hex};

/// A 2D point in layout space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }

    pub fn lerp(self, o: Point, t: f32) -> Point {
        Point::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

/// Forward (hex → pixel) and backward (pixel → hex) matrices plus the corner start angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orientation {
    pub f0: f32,
    pub f1: f32,
    pub f2: f32,
    pub f3: f32,
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub b3: f32,
    /// In multiples of 60°.
    pub start_angle: f32,
}

const SQRT3: f32 = 1.732_050_8;

impl Orientation {
    /// Hexes with a pointed top; rows are horizontal.
    pub const POINTY: Orientation = Orientation {
        f0: SQRT3,
        f1: SQRT3 / 2.0,
        f2: 0.0,
        f3: 1.5,
        b0: SQRT3 / 3.0,
        b1: -1.0 / 3.0,
        b2: 0.0,
        b3: 2.0 / 3.0,
        start_angle: 0.5,
    };

    /// Hexes with a flat top; columns are vertical.
    pub const FLAT: Orientation = Orientation {
        f0: 1.5,
        f1: 0.0,
        f2: SQRT3 / 2.0,
        f3: SQRT3,
        b0: 2.0 / 3.0,
        b1: 0.0,
        b2: -1.0 / 3.0,
        b3: SQRT3 / 3.0,
        start_angle: 0.0,
    };

    pub fn is_pointy(&self) -> bool {
        self.start_angle != 0.0
    }
}

/// Orientation + hex size + origin. `size` is the distance from centre to corner on each axis, so
/// non-uniform sizes stretch the grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub orientation: Orientation,
    pub size: Point,
    pub origin: Point,
}

impl Layout {
    pub const fn new(orientation: Orientation, size: Point, origin: Point) -> Self {
        Layout { orientation, size, origin }
    }

    pub const fn pointy(size: f32) -> Self {
        Layout::new(Orientation::POINTY, Point::new(size, size), Point::new(0.0, 0.0))
    }

    pub const fn flat(size: f32) -> Self {
        Layout::new(Orientation::FLAT, Point::new(size, size), Point::new(0.0, 0.0))
    }

    pub fn hex_to_point(&self, h: Hex) -> Point {
        self.frac_to_point(h.into())
    }

    pub fn frac_to_point(&self, h: FracHex) -> Point {
        let m = &self.orientation;
        let (q, r) = (h.q as f32, h.r as f32);
        Point::new(
            (m.f0 * q + m.f1 * r) * self.size.x + self.origin.x,
            (m.f2 * q + m.f3 * r) * self.size.y + self.origin.y,
        )
    }

    pub fn point_to_frac(&self, p: Point) -> FracHex {
        let m = &self.orientation;
        let x = (p.x - self.origin.x) / self.size.x;
        let y = (p.y - self.origin.y) / self.size.y;
        FracHex::new((m.b0 * x + m.b1 * y) as f64, (m.b2 * x + m.b3 * y) as f64)
    }

    pub fn point_to_hex(&self, p: Point) -> Hex {
        self.point_to_frac(p).round()
    }

    /// Offset of corner `i` (0..6) from the hex centre.
    pub fn corner_offset(&self, i: usize) -> Point {
        let angle = 2.0 * std::f32::consts::PI * (self.orientation.start_angle + i as f32) / 6.0;
        Point::new(self.size.x * angle.cos(), self.size.y * angle.sin())
    }

    pub fn corners(&self, h: Hex) -> [Point; 6] {
        let c = self.hex_to_point(h);
        std::array::from_fn(|i| {
            let o = self.corner_offset(i);
            Point::new(c.x + o.x, c.y + o.y)
        })
    }

    /// Direction index (see [`crate::DIRECTIONS`]) of the neighbour across the edge between
    /// corners `edge` and `edge + 1`.
    pub fn edge_direction(&self, edge: usize) -> usize {
        let a = self.corner_offset(edge % 6);
        let b = self.corner_offset((edge + 1) % 6);
        // The midpoint of an edge, pushed out to twice its distance, lands in the neighbour.
        let mid = Point::new(a.x + b.x + self.origin.x, a.y + b.y + self.origin.y);
        let n = self.point_to_hex(mid);
        Hex::ORIGIN.direction_to(n).expect("edge midpoint must fall in a neighbour")
    }

    /// For each direction, the edge index whose outward side faces it (inverse of
    /// [`Layout::edge_direction`]).
    pub fn direction_edges(&self) -> [usize; 6] {
        let mut out = [0; 6];
        for e in 0..6 {
            out[self.edge_direction(e)] = e;
        }
        out
    }

    /// Width and height of one hex's bounding box.
    pub fn hex_extent(&self) -> Point {
        if self.orientation.is_pointy() {
            Point::new(SQRT3 * self.size.x, 2.0 * self.size.y)
        } else {
            Point::new(2.0 * self.size.x, SQRT3 * self.size.y)
        }
    }

    /// Distance between the centres of two adjacent hexes (for uniform sizes).
    pub fn spacing(&self) -> f32 {
        SQRT3 * self.size.x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(layout: Layout) {
        for h in Hex::ORIGIN.spiral(4) {
            assert_eq!(layout.point_to_hex(layout.hex_to_point(h)), h);
            for c in layout.corners(h) {
                // Points slightly inside a corner still belong to the hex.
                let centre = layout.hex_to_point(h);
                assert_eq!(layout.point_to_hex(c.lerp(centre, 0.05)), h);
            }
        }
    }

    #[test]
    fn layouts_roundtrip() {
        roundtrip(Layout::pointy(1.0));
        roundtrip(Layout::flat(1.0));
        roundtrip(Layout::new(Orientation::FLAT, Point::new(10.0, 15.0), Point::new(35.0, 71.0)));
        roundtrip(Layout::new(Orientation::POINTY, Point::new(10.0, 15.0), Point::new(35.0, 71.0)));
    }

    #[test]
    fn edges_map_to_distinct_neighbours() {
        for layout in [Layout::pointy(1.0), Layout::flat(2.0)] {
            let mut seen = [false; 6];
            for e in 0..6 {
                seen[layout.edge_direction(e)] = true;
            }
            assert!(seen.iter().all(|&s| s));
            let de = layout.direction_edges();
            for (d, &e) in de.iter().enumerate() {
                assert_eq!(layout.edge_direction(e), d);
            }
        }
    }

    #[test]
    fn neighbour_spacing() {
        let l = Layout::pointy(1.0);
        let a = l.hex_to_point(Hex::ORIGIN);
        for d in 0..6 {
            let b = l.hex_to_point(Hex::direction(d));
            let dist = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            assert!((dist - l.spacing()).abs() < 1e-5);
        }
    }
}
