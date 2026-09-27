//! CPU-side RGBA images: decoding, procedural generation and simple edits.

use crate::Color;

/// An 8-bit RGBA image stored row by row, top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,
}

#[derive(Debug)]
pub struct ImageError(pub String);

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "image error: {}", self.0)
    }
}

impl std::error::Error for ImageError {}

impl Image {
    pub fn new(width: u32, height: u32) -> Image {
        Image { width, height, pixels: vec![[0; 4]; (width * height) as usize] }
    }

    pub fn filled(width: u32, height: u32, color: Color) -> Image {
        Image { width, height, pixels: vec![color.to_rgba8(); (width * height) as usize] }
    }

    /// Build an image by evaluating `f(x, y)` for each pixel.
    pub fn from_fn(width: u32, height: u32, mut f: impl FnMut(u32, u32) -> [u8; 4]) -> Image {
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                pixels.push(f(x, y));
            }
        }
        Image { width, height, pixels }
    }

    /// Decode a PNG file (any colour type / bit depth).
    pub fn from_png(bytes: &[u8]) -> Result<Image, ImageError> {
        let err = |e: png::DecodingError| ImageError(e.to_string());
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().map_err(err)?;
        let size = reader.output_buffer_size().ok_or_else(|| ImageError("png too large".into()))?;
        let mut buf = vec![0; size];
        let info = reader.next_frame(&mut buf).map_err(err)?;
        let data = &buf[..info.buffer_size()];
        let pixels: Vec<[u8; 4]> = match info.color_type {
            png::ColorType::Rgba => data.as_chunks::<4>().0.iter().map(|c| [c[0], c[1], c[2], c[3]]).collect(),
            png::ColorType::Rgb => data.as_chunks::<3>().0.iter().map(|c| [c[0], c[1], c[2], 255]).collect(),
            png::ColorType::GrayscaleAlpha => {
                data.as_chunks::<2>().0.iter().map(|c| [c[0], c[0], c[0], c[1]]).collect()
            }
            png::ColorType::Grayscale => data.iter().map(|&v| [v, v, v, 255]).collect(),
            png::ColorType::Indexed => return Err(ImageError("indexed png was not expanded".into())),
        };
        Ok(Image { width: info.width, height: info.height, pixels })
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        self.pixels[(y * self.width + x) as usize]
    }

    pub fn set(&mut self, x: u32, y: u32, v: [u8; 4]) {
        if x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = v;
        }
    }

    /// Copy a sub-rectangle.
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Image {
        Image::from_fn(w, h, |cx, cy| {
            let (sx, sy) = (x + cx, y + cy);
            if sx < self.width && sy < self.height { self.get(sx, sy) } else { [0; 4] }
        })
    }

    /// Split a horizontal strip into `frame_width`-wide frames.
    pub fn split_frames(&self, frame_width: u32) -> Vec<Image> {
        let n = (self.width / frame_width.max(1)).max(1);
        (0..n).map(|i| self.crop(i * frame_width, 0, frame_width, self.height)).collect()
    }

    pub fn flip_x(&self) -> Image {
        Image::from_fn(self.width, self.height, |x, y| self.get(self.width - 1 - x, y))
    }

    /// Scale up by an integer factor with nearest-neighbour sampling.
    pub fn upscale(&self, k: u32) -> Image {
        let k = k.max(1);
        Image::from_fn(self.width * k, self.height * k, |x, y| self.get(x / k, y / k))
    }

    /// Replace "key" pixels, for team colours. `map(pixel) -> Some(new)` recolours a pixel.
    pub fn recolor(&self, mut map: impl FnMut([u8; 4]) -> Option<[u8; 4]>) -> Image {
        Image {
            width: self.width,
            height: self.height,
            pixels: self.pixels.iter().map(|&p| map(p).unwrap_or(p)).collect(),
        }
    }

    /// Recolour magenta-keyed pixels (pure magenta hues, e.g. `#FF00FF`, `#C000C0`) to shades of
    /// `team`, keeping their brightness.
    pub fn recolor_magenta(&self, team: Color) -> Image {
        let t = team.to_rgba8();
        self.recolor(|p| {
            let is_key = p[3] > 0 && p[1] < 40 && p[0] > 60 && p[0].abs_diff(p[2]) < 24;
            is_key.then(|| {
                let k = p[0] as u32;
                let c = |v: u8| ((v as u32 * k) / 255).min(255) as u8;
                [c(t[0]), c(t[1]), c(t[2]), p[3]]
            })
        })
    }

    /// Draw `other` on top of this image at `(x, y)` with alpha blending.
    pub fn blit(&mut self, other: &Image, x: i32, y: i32) {
        for oy in 0..other.height {
            for ox in 0..other.width {
                let (tx, ty) = (x + ox as i32, y + oy as i32);
                if tx < 0 || ty < 0 || tx >= self.width as i32 || ty >= self.height as i32 {
                    continue;
                }
                let s = other.get(ox, oy);
                if s[3] == 0 {
                    continue;
                }
                let d = self.get(tx as u32, ty as u32);
                let a = s[3] as u32;
                let mix = |s: u8, d: u8| ((s as u32 * a + d as u32 * (255 - a)) / 255) as u8;
                let out =
                    [mix(s[0], d[0]), mix(s[1], d[1]), mix(s[2], d[2]), (a + d[3] as u32 * (255 - a) / 255) as u8];
                self.set(tx as u32, ty as u32, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_flip() {
        let img = Image::from_fn(8, 2, |x, y| [x as u8, y as u8, 0, 255]);
        let frames = img.split_frames(4);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[1].get(0, 1), [4, 1, 0, 255]);
        assert_eq!(img.flip_x().get(0, 0), [7, 0, 0, 255]);
        assert_eq!(img.upscale(2).width, 16);
    }

    #[test]
    fn recolor_magenta_keys() {
        let img = Image::from_fn(2, 1, |x, _| if x == 0 { [255, 0, 255, 255] } else { [10, 200, 10, 255] });
        let out = img.recolor_magenta(Color::rgb8(0, 0, 255));
        assert_eq!(out.get(0, 0), [0, 0, 255, 255]);
        assert_eq!(out.get(1, 0), [10, 200, 10, 255]);
    }
}
