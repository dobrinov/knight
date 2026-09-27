//! Procedural sound: sfxr-style effects and a tiny chiptune sequencer. Every game gets usable
//! placeholder audio without shipping a single file.

use knight_hex::Rng;

use crate::audio::Sound;

pub const RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    Square,
    Triangle,
    Saw,
    Sine,
    Noise,
}

/// One synthesized effect.
#[derive(Clone, Copy, Debug)]
pub struct Sfx {
    pub wave: Wave,
    /// Start frequency (Hz) and change per second (Hz/s) — negative slides down.
    pub freq: f32,
    pub slide: f32,
    /// Optional second pitch jump (as a ratio) after `jump_at` seconds (coins, power-ups).
    pub jump: f32,
    pub jump_at: f32,
    pub attack: f32,
    pub sustain: f32,
    pub decay: f32,
    pub vibrato: f32,
    pub volume: f32,
    /// Square duty cycle.
    pub duty: f32,
}

impl Default for Sfx {
    fn default() -> Self {
        Sfx {
            wave: Wave::Square,
            freq: 440.0,
            slide: 0.0,
            jump: 1.0,
            jump_at: 0.0,
            attack: 0.005,
            sustain: 0.05,
            decay: 0.1,
            vibrato: 0.0,
            volume: 0.5,
            duty: 0.5,
        }
    }
}

impl Sfx {
    pub fn duration(&self) -> f32 {
        self.attack + self.sustain + self.decay
    }

    pub fn render(&self, seed: u64) -> Sound {
        let n = (self.duration() * RATE as f32) as usize;
        let mut rng = Rng::new(seed);
        let mut phase = 0.0f32;
        let mut noise = 0.0f32;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let mut f = (self.freq + self.slide * t).max(20.0);
            if self.jump != 1.0 && t >= self.jump_at {
                f *= self.jump;
            }
            f *= 1.0 + self.vibrato * (t * 30.0).sin() * 0.05;
            let prev = phase;
            phase = (phase + f / RATE as f32).fract();
            if phase < prev {
                noise = rng.next_f32() * 2.0 - 1.0;
            }
            let s = match self.wave {
                Wave::Square => {
                    if phase < self.duty {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                Wave::Saw => phase * 2.0 - 1.0,
                Wave::Sine => (phase * std::f32::consts::TAU).sin(),
                Wave::Noise => noise,
            };
            let env = if t < self.attack {
                t / self.attack.max(1e-4)
            } else if t < self.attack + self.sustain {
                1.0
            } else {
                1.0 - (t - self.attack - self.sustain) / self.decay.max(1e-4)
            };
            out.push(s * env.max(0.0) * self.volume);
        }
        Sound::new(out, RATE)
    }

    // Presets.

    pub fn click() -> Sfx {
        Sfx {
            wave: Wave::Square,
            freq: 900.0,
            sustain: 0.01,
            decay: 0.03,
            volume: 0.25,
            duty: 0.25,
            ..Default::default()
        }
    }

    pub fn coin() -> Sfx {
        Sfx {
            wave: Wave::Square,
            freq: 988.0,
            jump: 1.335,
            jump_at: 0.06,
            sustain: 0.12,
            decay: 0.18,
            volume: 0.35,
            duty: 0.4,
            ..Default::default()
        }
    }

    pub fn hit() -> Sfx {
        Sfx {
            wave: Wave::Noise,
            freq: 900.0,
            slide: -2500.0,
            sustain: 0.03,
            decay: 0.12,
            volume: 0.5,
            ..Default::default()
        }
    }

    pub fn swing() -> Sfx {
        Sfx {
            wave: Wave::Noise,
            freq: 3000.0,
            slide: -9000.0,
            attack: 0.02,
            sustain: 0.02,
            decay: 0.08,
            volume: 0.25,
            ..Default::default()
        }
    }

    pub fn shoot() -> Sfx {
        Sfx {
            wave: Wave::Saw,
            freq: 1200.0,
            slide: -4000.0,
            sustain: 0.02,
            decay: 0.12,
            volume: 0.3,
            ..Default::default()
        }
    }

    pub fn explosion() -> Sfx {
        Sfx {
            wave: Wave::Noise,
            freq: 400.0,
            slide: -300.0,
            sustain: 0.1,
            decay: 0.6,
            volume: 0.6,
            ..Default::default()
        }
    }

    pub fn magic() -> Sfx {
        Sfx {
            wave: Wave::Triangle,
            freq: 300.0,
            slide: 1800.0,
            sustain: 0.2,
            decay: 0.3,
            vibrato: 1.0,
            volume: 0.4,
            ..Default::default()
        }
    }

    pub fn step() -> Sfx {
        Sfx {
            wave: Wave::Noise,
            freq: 180.0,
            slide: -200.0,
            sustain: 0.01,
            decay: 0.05,
            volume: 0.15,
            ..Default::default()
        }
    }

    pub fn pop() -> Sfx {
        Sfx {
            wave: Wave::Sine,
            freq: 600.0,
            slide: 2400.0,
            sustain: 0.02,
            decay: 0.06,
            volume: 0.3,
            ..Default::default()
        }
    }

    pub fn powerup() -> Sfx {
        Sfx {
            wave: Wave::Square,
            freq: 440.0,
            slide: 900.0,
            jump: 1.5,
            jump_at: 0.15,
            sustain: 0.25,
            decay: 0.2,
            volume: 0.3,
            duty: 0.3,
            ..Default::default()
        }
    }

    pub fn deny() -> Sfx {
        Sfx {
            wave: Wave::Square,
            freq: 180.0,
            slide: -60.0,
            sustain: 0.1,
            decay: 0.08,
            volume: 0.3,
            duty: 0.5,
            ..Default::default()
        }
    }
}

fn note_hz(semitones_from_a4: i32) -> f32 {
    440.0 * 2f32.powf(semitones_from_a4 as f32 / 12.0)
}

/// Render a looping chiptune: lead melody, bass and a noise hi-hat, `bars` bars of 4 beats.
/// Deterministic per `seed`.
pub fn chiptune(seed: u64, bpm: f32, bars: usize) -> Sound {
    let mut rng = Rng::new(seed);
    let beat = 60.0 / bpm;
    let eighth = beat / 2.0;
    let total = bars * 8;
    let n = (total as f32 * eighth * RATE as f32) as usize;
    let mut out = vec![0.0f32; n];
    // A minor pentatonic-ish scale around A3..A5 and a i-VI-III-VII progression.
    let scale = [0, 3, 5, 7, 10, 12, 15, 17];
    let roots = [-12, -16, -9, -14];
    let mut melody = Vec::with_capacity(total);
    let mut idx = 3i32;
    for k in 0..total {
        if k % 8 == 0 || rng.chance(0.55) {
            idx = (idx + rng.range(-2, 3)).clamp(0, scale.len() as i32 - 1);
            melody.push(Some(scale[idx as usize]));
        } else {
            melody.push(if rng.chance(0.4) { None } else { melody.last().copied().flatten() });
        }
    }
    let mut add = |start: f32, dur: f32, f: f32, wave: Wave, vol: f32| {
        let s0 = (start * RATE as f32) as usize;
        let len = (dur * RATE as f32) as usize;
        let mut phase = 0.0f32;
        for i in 0..len {
            let Some(o) = out.get_mut(s0 + i) else { break };
            phase = (phase + f / RATE as f32).fract();
            let s = match wave {
                Wave::Square => {
                    if phase < 0.35 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                _ => (phase * std::f32::consts::TAU).sin(),
            };
            let t = i as f32 / len as f32;
            let env = (1.0 - t).powf(1.5) * (i as f32 / 200.0).min(1.0);
            *o += s * env * vol;
        }
    };
    for (k, note) in melody.iter().enumerate() {
        let t = k as f32 * eighth;
        if let Some(n) = note {
            add(t, eighth * 0.9, note_hz(*n), Wave::Square, 0.09);
        }
        let root = roots[(k / 8) % roots.len()];
        if k % 2 == 0 {
            add(t, eighth * 1.8, note_hz(root - 12), Wave::Triangle, 0.18);
        }
    }
    // Hi-hat on off-beats, kick on beats.
    let mut noise = Rng::new(seed ^ 0xabc);
    for k in 0..total {
        let s0 = (k as f32 * eighth * RATE as f32) as usize;
        if k % 2 == 1 {
            for i in 0..1200 {
                if let Some(o) = out.get_mut(s0 + i) {
                    *o += (noise.next_f32() * 2.0 - 1.0) * 0.05 * (1.0 - i as f32 / 1200.0);
                }
            }
        } else if k % 4 == 0 {
            for i in 0..5000 {
                if let Some(o) = out.get_mut(s0 + i) {
                    let t = i as f32 / RATE as f32;
                    *o += (t * (60.0 + 200.0 * (-t * 40.0).exp()) * std::f32::consts::TAU).sin()
                        * 0.25
                        * (1.0 - i as f32 / 5000.0);
                }
            }
        }
    }
    Sound::new(out, RATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_render_bounded_audio() {
        for sfx in [Sfx::click(), Sfx::coin(), Sfx::hit(), Sfx::explosion(), Sfx::magic(), Sfx::powerup()] {
            let s = sfx.render(1);
            assert!(!s.samples.is_empty());
            assert!(s.samples.iter().all(|v| v.abs() <= 1.0));
            assert!(s.samples.iter().any(|v| v.abs() > 0.05));
        }
        let m = chiptune(3, 120.0, 4);
        assert!((m.duration() - 8.0).abs() < 0.05);
        assert!(m.samples.iter().all(|v| v.is_finite()));
    }
}
