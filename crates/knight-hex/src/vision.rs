//! Height-aware line of sight and field of view.

use crate::{FracHex, Hex, HexSet};

/// Can an eye at `eye` above `from`'s surface see a point `target` above `to`'s surface?
///
/// `height(h)` returns the blocking height of a hex (terrain plus walls, trees, ...). The sight
/// line is sampled at every hex it crosses; each end is excluded. Like [`Hex::line_to`] it nudges
/// the line slightly so rays along edges resolve consistently; `lenient` also tries the opposite
/// nudge and accepts either, which makes edge-grazing lines symmetric and a bit more permissive.
pub fn line_of_sight(from: Hex, to: Hex, eye: f32, target: f32, lenient: bool, height: impl Fn(Hex) -> f32) -> bool {
    let clear = |nudge: f64| {
        let n = from.distance(to);
        if n <= 1 {
            return true;
        }
        let a = FracHex::new(from.q as f64 + nudge, from.r as f64 + nudge);
        let b = FracHex::new(to.q as f64 + nudge, to.r as f64 + nudge);
        let h0 = height(from) + eye;
        let h1 = height(to) + target;
        (1..n).all(|i| {
            let t = i as f64 / n as f64;
            let h = a.lerp(b, t).round();
            height(h) <= h0 + (h1 - h0) * t as f32
        })
    };
    clear(1e-6) || (lenient && clear(-1e-6))
}

/// Every hex within `radius` of `origin` whose surface is visible from an eye `eye` above the
/// origin's surface. `height` works as in [`line_of_sight`].
pub fn field_of_view(origin: Hex, radius: i32, eye: f32, height: impl Fn(Hex) -> f32) -> HexSet {
    origin.range(radius).filter(|&h| line_of_sight(origin, h, eye, 0.05, true, &height)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_ground_sees_everything() {
        let fov = field_of_view(Hex::ORIGIN, 5, 1.0, |_| 0.0);
        assert_eq!(fov.len(), 91);
    }

    #[test]
    fn wall_blocks() {
        let wall = |h: Hex| if h == Hex::new(1, 0) { 10.0 } else { 0.0 };
        assert!(!line_of_sight(Hex::ORIGIN, Hex::new(3, 0), 1.0, 0.0, true, wall));
        assert!(line_of_sight(Hex::ORIGIN, Hex::new(0, 3), 1.0, 0.0, true, wall));
        let fov = field_of_view(Hex::ORIGIN, 4, 1.0, wall);
        assert!(!fov.contains(&Hex::new(4, 0)));
        assert!(fov.contains(&Hex::new(1, 0)));
    }

    #[test]
    fn high_ground_sees_over_low_walls() {
        let h = |x: Hex| match x {
            Hex { q: 0, r: 0 } => 5.0,
            Hex { q: 1, r: 0 } => 2.0,
            _ => 0.0,
        };
        assert!(line_of_sight(Hex::ORIGIN, Hex::new(3, 0), 1.0, 0.0, false, h));
        // ...but the low ground cannot see the hill top past the wall.
        let valley = |x: Hex| if x == Hex::new(1, 0) { 2.0 } else { 0.0 };
        assert!(!line_of_sight(Hex::ORIGIN, Hex::new(3, 0), 0.5, 0.0, false, valley));
    }
}
