//! Deterministic randomness: a small PRNG and seeded value noise for terrain generation.

/// SplitMix64-seeded xorshift* generator. Deterministic across platforms, good enough for games.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: splitmix(seed).max(1) }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in `[lo, hi)`. Returns `lo` when the range is empty.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo) as u64) as i32
    }

    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() { None } else { Some(&items[self.range(0, items.len() as i32) as usize]) }
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.range(0, i as i32 + 1) as usize;
            items.swap(i, j);
        }
    }
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Hash two integers and a seed to a float in `[0, 1)`.
pub fn hash2(seed: u64, x: i32, y: i32) -> f32 {
    let h = splitmix(seed ^ splitmix((x as u32 as u64) << 32 | y as u32 as u64));
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Seeded 2D value noise with smooth interpolation and fractal (fBm) layering.
#[derive(Clone, Copy, Debug)]
pub struct ValueNoise {
    pub seed: u64,
}

impl ValueNoise {
    pub const fn new(seed: u64) -> Self {
        ValueNoise { seed }
    }

    /// Noise in `[0, 1)`, with features roughly one unit wide.
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (ix, iy) = (x0 as i32, y0 as i32);
        let s = |t: f32| t * t * (3.0 - 2.0 * t);
        let (sx, sy) = (s(fx), s(fy));
        let v00 = hash2(self.seed, ix, iy);
        let v10 = hash2(self.seed, ix + 1, iy);
        let v01 = hash2(self.seed, ix, iy + 1);
        let v11 = hash2(self.seed, ix + 1, iy + 1);
        let a = v00 + (v10 - v00) * sx;
        let b = v01 + (v11 - v01) * sx;
        a + (b - a) * sy
    }

    /// Fractal Brownian motion: `octaves` layers, each at double frequency and half amplitude.
    /// Normalised to `[0, 1)`.
    pub fn fbm(&self, x: f32, y: f32, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for o in 0..octaves.max(1) {
            let layer = ValueNoise::new(self.seed.wrapping_add(o as u64 * 7919));
            sum += layer.sample(x * freq, y * freq) * amp;
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
            let f = a.next_f32();
            assert!((0.0..1.0).contains(&f));
            let r = a.range(-3, 4);
            assert!((-3..4).contains(&r));
            b.next_f32();
            b.range(-3, 4);
        }
    }

    #[test]
    fn noise_is_smooth_and_bounded() {
        let n = ValueNoise::new(7);
        let mut prev = n.sample(0.0, 0.3);
        for i in 1..400 {
            let v = n.fbm(i as f32 * 0.01, 0.3, 4);
            assert!((0.0..1.0).contains(&v));
            let s = n.sample(i as f32 * 0.01, 0.3);
            assert!((s - prev).abs() < 0.1);
            prev = s;
        }
        assert_eq!(n.sample(3.0, 4.0), hash2(7, 3, 4));
    }
}
