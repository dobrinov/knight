//! Particles: short-lived glowing or smoky specks for spells, fire, dust and fireworks.

use glam::{Vec2, Vec3};
use knight_hex::Rng;

use crate::Color;
use crate::frame::{Sprite, WorldDraw};

#[derive(Clone, Copy, Debug)]
struct Particle {
    pos: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    size: f32,
    grow: f32,
    from: Color,
    to: Color,
    gravity: f32,
    drag: f32,
    glow: bool,
}

/// Parameters for [`Particles::burst`].
#[derive(Clone, Copy, Debug)]
pub struct Burst {
    pub count: u32,
    /// Initial speed range (world units per second).
    pub speed: (f32, f32),
    /// Extra upward speed.
    pub lift: f32,
    /// Horizontal spread 0 (straight up) .. 1 (all directions).
    pub spread: f32,
    pub life: (f32, f32),
    /// Diameter at birth (world units) and growth per second.
    pub size: f32,
    pub grow: f32,
    /// Colour at birth and at death (alpha fades with it).
    pub from: Color,
    pub to: Color,
    pub gravity: f32,
    pub drag: f32,
    /// Drawn unlit on top (sparks, magic) instead of lit and depth-tested (smoke, dust).
    pub glow: bool,
}

impl Burst {
    /// Bright sparks flying out and falling.
    pub fn sparks(color: Color) -> Burst {
        Burst {
            count: 40,
            speed: (1.5, 4.0),
            lift: 2.0,
            spread: 1.0,
            life: (0.5, 1.1),
            size: 0.14,
            grow: -0.08,
            from: color,
            to: color.with_alpha(0.0),
            gravity: 5.0,
            drag: 0.8,
            glow: true,
        }
    }

    /// Rising, growing, fading smoke.
    pub fn smoke() -> Burst {
        Burst {
            count: 6,
            speed: (0.1, 0.4),
            lift: 0.8,
            spread: 0.4,
            life: (1.5, 2.5),
            size: 0.35,
            grow: 0.5,
            from: Color::rgba(0.35, 0.33, 0.32, 0.55),
            to: Color::rgba(0.6, 0.6, 0.6, 0.0),
            gravity: -0.1,
            drag: 0.5,
            glow: false,
        }
    }

    /// Flickering flame (emit a few every frame).
    pub fn fire() -> Burst {
        Burst {
            count: 3,
            speed: (0.05, 0.3),
            lift: 1.2,
            spread: 0.3,
            life: (0.4, 0.8),
            size: 0.3,
            grow: -0.3,
            from: Color::hex(0xffd060),
            to: Color::rgba(0.9, 0.2, 0.05, 0.0),
            gravity: -0.5,
            drag: 1.0,
            glow: true,
        }
    }
}

/// A particle system. Emit, update once per frame, draw into a world pass.
#[derive(Clone, Debug)]
pub struct Particles {
    list: Vec<Particle>,
    rng: Rng,
    /// Hard cap; the oldest particles make room for new ones.
    pub max: usize,
}

impl Default for Particles {
    fn default() -> Self {
        Particles { list: Vec::new(), rng: Rng::new(0x5eed), max: 4000 }
    }
}

impl Particles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn burst(&mut self, at: Vec3, b: &Burst) {
        for _ in 0..b.count {
            let a = self.rng.range_f32(0.0, std::f32::consts::TAU);
            let speed = self.rng.range_f32(b.speed.0, b.speed.1);
            let flat = Vec2::new(a.cos(), a.sin()) * b.spread;
            let dir = Vec3::new(flat.x, 1.0 - b.spread * 0.5, flat.y).normalize_or_zero();
            if self.list.len() >= self.max {
                // Order is irrelevant for drawing, so the cheap removal will do.
                self.list.swap_remove(0);
            }
            self.list.push(Particle {
                pos: at,
                vel: dir * speed + Vec3::Y * b.lift,
                age: 0.0,
                life: self.rng.range_f32(b.life.0, b.life.1),
                size: b.size,
                grow: b.grow,
                from: b.from,
                to: b.to,
                gravity: b.gravity,
                drag: b.drag,
                glow: b.glow,
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.list {
            p.age += dt;
            p.vel.y -= p.gravity * dt;
            p.vel *= (1.0 - p.drag * dt).max(0.0);
            p.pos += p.vel * dt;
            p.size = (p.size + p.grow * dt).max(0.01);
        }
        self.list.retain(|p| p.age < p.life);
    }

    pub fn draw(&self, w: &mut WorldDraw) {
        let circle = w.assets.builtin.circle;
        let px = w.assets.region(circle).width as f32 / w.world.pixels_per_unit;
        for p in &self.list {
            let t = p.age / p.life;
            let c = p.from.lerp(p.to, t);
            let s = Sprite::new(circle, p.pos).anchor(0.5, 0.5).scale(p.size / px).tint(c);
            if p.glow {
                w.sprite_on_top(s);
            } else {
                w.sprite(Sprite { translucent: true, ..s });
            }
        }
    }
}
