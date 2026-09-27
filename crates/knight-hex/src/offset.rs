//! Offset and doubled coordinates, handy for rectangular maps and for storing maps in 2D arrays.

use crate::Hex;

/// Which rows (or columns) are shoved by half a hex.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Parity {
    Odd,
    Even,
}

impl Parity {
    const fn sign(self) -> i32 {
        match self {
            Parity::Odd => -1,
            Parity::Even => 1,
        }
    }
}

/// A `(col, row)` coordinate in an offset grid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Offset {
    pub col: i32,
    pub row: i32,
}

impl Offset {
    pub const fn new(col: i32, row: i32) -> Self {
        Offset { col, row }
    }

    /// Offset rows ("r" layouts, used with pointy hexes).
    pub fn from_hex_r(h: Hex, parity: Parity) -> Offset {
        Offset::new(h.q + (h.r + parity.sign() * (h.r & 1)) / 2, h.r)
    }

    pub fn to_hex_r(self, parity: Parity) -> Hex {
        Hex::new(self.col - (self.row + parity.sign() * (self.row & 1)) / 2, self.row)
    }

    /// Offset columns ("q" layouts, used with flat hexes).
    pub fn from_hex_q(h: Hex, parity: Parity) -> Offset {
        Offset::new(h.q, h.r + (h.q + parity.sign() * (h.q & 1)) / 2)
    }

    pub fn to_hex_q(self, parity: Parity) -> Hex {
        Hex::new(self.col, self.row - (self.col + parity.sign() * (self.col & 1)) / 2)
    }
}

/// Doubled coordinates: one axis steps by two so that neighbours stay simple.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Doubled {
    pub col: i32,
    pub row: i32,
}

impl Doubled {
    pub const fn new(col: i32, row: i32) -> Self {
        Doubled { col, row }
    }

    /// Doubled-width (pointy hexes).
    pub const fn from_hex_width(h: Hex) -> Doubled {
        Doubled::new(2 * h.q + h.r, h.r)
    }

    pub const fn to_hex_width(self) -> Hex {
        Hex::new((self.col - self.row) / 2, self.row)
    }

    /// Doubled-height (flat hexes).
    pub const fn from_hex_height(h: Hex) -> Doubled {
        Doubled::new(h.q, 2 * h.r + h.q)
    }

    pub const fn to_hex_height(self) -> Hex {
        Hex::new(self.col, (self.row - self.col) / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_roundtrip() {
        for h in Hex::ORIGIN.spiral(5) {
            for p in [Parity::Odd, Parity::Even] {
                assert_eq!(Offset::from_hex_r(h, p).to_hex_r(p), h);
                assert_eq!(Offset::from_hex_q(h, p).to_hex_q(p), h);
            }
            assert_eq!(Doubled::from_hex_width(h).to_hex_width(), h);
            assert_eq!(Doubled::from_hex_height(h).to_hex_height(), h);
        }
    }

    #[test]
    fn red_blob_examples() {
        let a = Hex::new(1, 2);
        let b = Hex::new(-1, -2);
        assert_eq!(Offset::from_hex_q(a, Parity::Even), Offset::new(1, 3));
        assert_eq!(Offset::from_hex_q(b, Parity::Even), Offset::new(-1, -2));
        assert_eq!(Offset::from_hex_q(a, Parity::Odd), Offset::new(1, 2));
        assert_eq!(Offset::from_hex_q(b, Parity::Odd), Offset::new(-1, -3));
        assert_eq!(Offset::from_hex_r(a, Parity::Even), Offset::new(2, 2));
        assert_eq!(Offset::from_hex_r(b, Parity::Even), Offset::new(-2, -2));
        assert_eq!(Offset::from_hex_r(a, Parity::Odd), Offset::new(2, 2));
        assert_eq!(Offset::from_hex_r(b, Parity::Odd), Offset::new(-2, -2));
    }
}
