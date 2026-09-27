//! sRGB colours. Knight renders in sRGB space end to end (like most 2D and pixel-art engines), so
//! these values are what you see on screen.

/// An RGBA colour with components in `0..=1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    pub const RED: Color = Color::rgb(0.9, 0.2, 0.2);
    pub const GREEN: Color = Color::rgb(0.3, 0.8, 0.3);
    pub const BLUE: Color = Color::rgb(0.25, 0.45, 0.9);
    pub const YELLOW: Color = Color::rgb(1.0, 0.85, 0.25);
    pub const GRAY: Color = Color::rgb(0.5, 0.5, 0.5);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    /// From a `0xRRGGBB` literal.
    pub const fn hex(v: u32) -> Color {
        Color::rgb(((v >> 16) & 0xff) as f32 / 255.0, ((v >> 8) & 0xff) as f32 / 255.0, (v & 0xff) as f32 / 255.0)
    }

    pub const fn rgb8(r: u8, g: u8, b: u8) -> Color {
        Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    pub const fn with_alpha(self, a: f32) -> Color {
        Color { a, ..self }
    }

    /// Multiply RGB by `k` (alpha unchanged).
    pub fn shade(self, k: f32) -> Color {
        Color { r: self.r * k, g: self.g * k, b: self.b * k, a: self.a }
    }

    pub fn lerp(self, o: Color, t: f32) -> Color {
        Color {
            r: self.r + (o.r - self.r) * t,
            g: self.g + (o.g - self.g) * t,
            b: self.b + (o.b - self.b) * t,
            a: self.a + (o.a - self.a) * t,
        }
    }

    /// Component-wise multiply (tinting).
    pub fn tint(self, o: Color) -> Color {
        Color { r: self.r * o.r, g: self.g * o.g, b: self.b * o.b, a: self.a * o.a }
    }

    /// Multiply RGB by another colour's RGB (alpha unchanged).
    pub fn mul_rgb(self, o: Color) -> Color {
        Color { r: self.r * o.r, g: self.g * o.g, b: self.b * o.b, a: self.a }
    }

    pub fn luma(self) -> f32 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }

    pub fn to_rgba8(self) -> [u8; 4] {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        [c(self.r), c(self.g), c(self.b), c(self.a)]
    }

    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl From<[u8; 4]> for Color {
    fn from(v: [u8; 4]) -> Self {
        Color::rgba(v[0] as f32 / 255.0, v[1] as f32 / 255.0, v[2] as f32 / 255.0, v[3] as f32 / 255.0)
    }
}
