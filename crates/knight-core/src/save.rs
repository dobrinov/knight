//! Saving and loading a [`HexWorld`]'s map data: layout, height step, base height, water level
//! and colour, tiles (heights and materials, by material name) and fog of war.
//!
//! The format is a small self-contained binary blob (`KHEX` + version), so games can put it in
//! a file, a browser's storage or a network message. Materials are stored by name and looked up
//! in the loading world with [`HexWorld::material_by_name`]; names it does not have fall back
//! to material 0 (textures and movement costs are the game's, not the map's). Custom
//! [`knight_hex::Orientation`]s are saved as pointy or flat.

use knight_hex::{Hex, Layout, Point};

use crate::Color;
use crate::world::{HexWorld, Tile, Visibility, Water};

const MAGIC: &[u8; 4] = b"KHEX";
const VERSION: u8 = 1;

/// Why a blob could not be loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveError(pub &'static str);

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "map data: {}", self.0)
    }
}

impl std::error::Error for SaveError {}

// --- A tiny varint codec (kept private so `knight-core` stays dependency-free) ----------------

fn put_var(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn put_ivar(out: &mut Vec<u8>, v: i64) {
    put_var(out, ((v << 1) ^ (v >> 63)) as u64);
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_var(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], SaveError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.buf.len()).ok_or(SaveError("unexpected end"))?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, SaveError> {
        Ok(self.take(1)?[0])
    }

    fn var(&mut self) -> Result<u64, SaveError> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(SaveError("varint too long"))
    }

    fn ivar(&mut self) -> Result<i64, SaveError> {
        let v = self.var()?;
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }

    fn i32(&mut self) -> Result<i32, SaveError> {
        i32::try_from(self.ivar()?).map_err(|_| SaveError("value out of range"))
    }

    fn f32(&mut self) -> Result<f32, SaveError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn str(&mut self) -> Result<String, SaveError> {
        let n = self.len()?;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| SaveError("invalid utf-8"))
    }

    /// A length that must fit in what is left (each item needs at least a byte).
    fn len(&mut self) -> Result<usize, SaveError> {
        let n = self.var()? as usize;
        if n > self.buf.len() - self.pos {
            return Err(SaveError("length too large"));
        }
        Ok(n)
    }

    fn hexes(&mut self) -> Result<Vec<Hex>, SaveError> {
        let n = self.len()?;
        let (mut q, mut r) = (0, 0);
        (0..n)
            .map(|_| {
                q += self.i32()?;
                r += self.i32()?;
                Ok(Hex::new(q, r))
            })
            .collect()
    }
}

/// Sorted hexes, delta-encoded (neighbouring coordinates pack into a byte or two).
fn put_hexes(out: &mut Vec<u8>, hexes: &mut [Hex]) {
    hexes.sort();
    put_var(out, hexes.len() as u64);
    let (mut q, mut r) = (0, 0);
    for h in hexes {
        put_ivar(out, (h.q - q) as i64);
        put_ivar(out, (h.r - r) as i64);
        (q, r) = (h.q, h.r);
    }
}

impl HexWorld {
    /// Serialize the map (see the [module docs](self)).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + self.len() * 4);
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        // Layout and vertical scale.
        out.push(self.layout.orientation.is_pointy() as u8);
        for v in [self.layout.size.x, self.layout.size.y, self.layout.origin.x, self.layout.origin.y] {
            put_f32(&mut out, v);
        }
        put_f32(&mut out, self.height_step);
        put_ivar(&mut out, self.base_height as i64);
        // Water.
        match &self.water {
            Some(w) => {
                out.push(1);
                put_ivar(&mut out, w.level as i64);
                out.extend_from_slice(&w.color.to_rgba8());
            }
            None => out.push(0),
        }
        // Materials by name.
        put_var(&mut out, self.materials.len() as u64);
        for m in &self.materials {
            put_str(&mut out, &m.name);
        }
        // Tiles, sorted and delta-encoded.
        let mut tiles: Vec<(Hex, Tile)> = self.tiles().collect();
        tiles.sort_by_key(|(h, _)| *h);
        put_var(&mut out, tiles.len() as u64);
        let (mut q, mut r) = (0, 0);
        for (h, t) in tiles {
            put_ivar(&mut out, (h.q - q) as i64);
            put_ivar(&mut out, (h.r - r) as i64);
            put_ivar(&mut out, t.height as i64);
            put_var(&mut out, t.material as u64);
            (q, r) = (h.q, h.r);
        }
        // Fog of war: explored and visible hexes.
        out.push(self.fog_enabled() as u8);
        if self.fog_enabled() {
            let mut explored: Vec<Hex> = self.hexes().filter(|&h| self.visibility(h) != Visibility::Hidden).collect();
            put_hexes(&mut out, &mut explored);
            let mut visible: Vec<Hex> = self.visible_hexes().collect();
            put_hexes(&mut out, &mut visible);
        }
        out
    }

    /// Replace this world's map with one saved by [`HexWorld::to_bytes`]. Materials are matched
    /// by name against [`HexWorld::materials`] (add them before loading); unknown names become
    /// material 0. On error the world is left unchanged.
    pub fn load_bytes(&mut self, bytes: &[u8]) -> Result<(), SaveError> {
        let mut r = Reader { buf: bytes, pos: 0 };
        if r.take(4)? != MAGIC {
            return Err(SaveError("not a Knight map"));
        }
        if r.u8()? != VERSION {
            return Err(SaveError("unsupported version"));
        }
        let pointy = r.u8()? != 0;
        let (sx, sy, ox, oy) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
        let mut layout = if pointy { Layout::pointy(1.0) } else { Layout::flat(1.0) };
        layout.size = Point::new(sx, sy);
        layout.origin = Point::new(ox, oy);
        let height_step = r.f32()?;
        let base_height = i16::try_from(r.ivar()?).map_err(|_| SaveError("base height out of range"))?;
        let water = match r.u8()? {
            0 => None,
            _ => {
                let level = i16::try_from(r.ivar()?).map_err(|_| SaveError("water level out of range"))?;
                let c = r.take(4)?;
                Some((level, Color::from([c[0], c[1], c[2], c[3]])))
            }
        };
        let names: Vec<String> = (0..r.len()?).map(|_| r.str()).collect::<Result<_, _>>()?;
        let mut missing = Vec::new();
        let materials: Vec<u16> = names
            .iter()
            .map(|n| {
                self.material_by_name(n).unwrap_or_else(|| {
                    missing.push(n.as_str());
                    0
                })
            })
            .collect();
        let n = r.len()?;
        let mut tiles = Vec::with_capacity(n);
        let (mut q, mut rr) = (0, 0);
        for _ in 0..n {
            q += r.i32()?;
            rr += r.i32()?;
            let height = i16::try_from(r.ivar()?).map_err(|_| SaveError("height out of range"))?;
            let material = *materials.get(r.var()? as usize).ok_or(SaveError("bad material index"))?;
            tiles.push((Hex::new(q, rr), Tile::new(height, material)));
        }
        let fog = match r.u8()? {
            0 => None,
            _ => Some((r.hexes()?, r.hexes()?)),
        };
        if r.pos != bytes.len() {
            return Err(SaveError("trailing data"));
        }
        if !missing.is_empty() {
            log::warn!("knight: map uses unknown materials {missing:?}; using material 0");
        }

        // Everything parsed: apply.
        self.clear();
        self.layout = layout;
        self.height_step = height_step;
        self.base_height = base_height;
        self.water =
            water.map(|(level, color)| Water { level, color, texture: self.water.as_ref().and_then(|w| w.texture) });
        for (h, t) in tiles {
            self.set_tile(h, t);
        }
        self.set_fog_enabled(fog.is_some());
        if let Some((explored, visible)) = fog {
            for h in explored {
                self.set_visibility(h, Visibility::Explored);
            }
            self.update_fog(visible);
        }
        self.mark_all_dirty();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Material;
    use knight_hex::shapes;

    #[test]
    fn roundtrip() {
        let mut w = HexWorld::new(Layout::flat(1.5));
        w.layout.origin = Point::new(3.0, -2.0);
        w.height_step = 0.5;
        w.base_height = -3;
        let grass = w.add_material(Material::color("grass", Color::GREEN, Color::GRAY));
        let rock = w.add_material(Material::color("rock", Color::GRAY, Color::GRAY));
        w.water = Some(Water { level: 1, color: Color::BLUE, texture: None });
        for (i, h) in shapes::hexagon(Hex::new(40, -90), 9).into_iter().enumerate() {
            w.set_tile(h, Tile::new((i % 7) as i16 - 2, if i % 3 == 0 { rock } else { grass }));
        }
        w.set_fog_enabled(true);
        w.update_fog(Hex::new(40, -90).range(3));
        w.update_fog(Hex::new(42, -90).range(2));
        let bytes = w.to_bytes();
        assert!(bytes.len() < w.len() * 6, "tiles pack into a few bytes each: {}", bytes.len());

        // Load into a world that has the materials in another order (and one extra).
        let mut v = HexWorld::new(Layout::pointy(1.0));
        v.add_material(Material::color("rock", Color::GRAY, Color::GRAY));
        v.add_material(Material::color("mud", Color::GRAY, Color::GRAY));
        v.add_material(Material::color("grass", Color::GREEN, Color::GRAY));
        v.load_bytes(&bytes).unwrap();
        assert_eq!(v.layout, w.layout);
        assert_eq!((v.height_step, v.base_height), (0.5, -3));
        assert_eq!(v.water.as_ref().map(|x| x.level), Some(1));
        assert_eq!(v.len(), w.len());
        for (h, t) in w.tiles() {
            let got = v.tile(h).unwrap();
            assert_eq!(got.height, t.height);
            assert_eq!(v.materials[got.material as usize].name, w.materials[t.material as usize].name);
            assert_eq!(v.visibility(h), w.visibility(h), "{h}");
        }
    }

    #[test]
    fn rejects_garbage_and_keeps_the_world() {
        let mut w = HexWorld::new(Layout::pointy(1.0));
        w.set_tile(Hex::ORIGIN, Tile::new(2, 0));
        assert!(w.load_bytes(b"nope").is_err());
        let mut bytes = w.to_bytes();
        bytes.truncate(bytes.len() - 2);
        assert!(w.load_bytes(&bytes).is_err());
        bytes = w.to_bytes();
        bytes.push(0);
        assert_eq!(w.load_bytes(&bytes), Err(SaveError("trailing data")));
        assert_eq!(w.tile(Hex::ORIGIN), Some(Tile::new(2, 0)));
    }
}
