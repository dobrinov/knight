//! The Knight asset pack: pixel-art terrain, props, fire, and humans / orcs / elves units,
//! heroes, monsters and buildings, bundled into the binary and loaded on demand.
//!
//! Everything is drawn at [`ART_PIXELS_PER_UNIT`] (16 art pixels per world unit, a hex is 28 × 32
//! px) with one 48-colour palette. Team colour is painted in four magenta shades and recoloured
//! when loading, so the same sheet serves every player.
//!
//! ```ignore
//! let mut pack = Pack::new();
//! world.pixels_per_unit = 32.0;                 // any value; sprites scale with `pack.scale(&world)`
//! let grass = world.add_material(pack.material(&mut ctx.assets, "grass"));
//! let footman = pack.character(&mut ctx.assets, "unit_human_footman", Some(Color::hex(0x3d7be0)));
//! w.sprite(Sprite::new(footman.walk.frame_at(t), pos).scale(pack.scale(&w.world)));
//! ```
//!
//! Sheets are cut and uploaded the first time they are asked for (and cached per team colour),
//! so a game only pays atlas space for what it uses. The atlas grows extra pages when needed.

use std::collections::HashMap;

use knight_core::glam::Vec2;
use knight_core::{Animation, Assets, Color, HexWorld, Image, ImageId, Material};

mod catalog;
pub use catalog::{CHARACTERS, FX_FIRE, PROPS, STRUCTURES, STYLE_REFERENCE, TERRAIN};

/// Art pixels per world unit in every sheet of the pack.
pub const ART_PIXELS_PER_UNIT: f32 = 16.0;

/// Colour given to team-coloured parts when no team is asked for.
pub const NEUTRAL_TEAM: Color = Color::hex(0xc8a45c);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Faction {
    Human,
    Orc,
    Elf,
    Neutral,
    Monster,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Unit,
    Hero,
    Monster,
}

/// What the sixth row of a character sheet holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Special {
    /// A second, heavier melee attack.
    Attack2,
    /// A ranged shot (the projectile is not in the sheet).
    Shoot,
    /// A spell cast.
    Cast,
    /// Workers chopping / mining (4 frames, loops).
    Work,
}

#[derive(Clone, Copy, Debug)]
pub struct CharacterInfo {
    /// File stem, e.g. `unit_human_footman`.
    pub id: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub faction: Faction,
    /// Cell size in pixels (48, 64 or 96).
    pub cell: u32,
    pub special: Special,
    /// Drawn hovering above the ground (give it a shadow on the ground).
    pub flying: bool,
    pub png: &'static [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct StructureInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub faction: Faction,
    pub cell: (u32, u32),
    pub png: &'static [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct TerrainInfo {
    /// File stem, e.g. `terrain_grass`.
    pub id: &'static str,
    /// Short name, e.g. `grass` (what [`Pack::material`] takes).
    pub name: &'static str,
    pub animated: bool,
    pub png: &'static [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct PropInfo {
    pub id: &'static str,
    pub cell: (u32, u32),
    pub png: &'static [u8],
}

/// Animations of one character (in one team colour). Frames share one size and are anchored at
/// the bottom centre (the feet), so draw them with the default sprite anchor.
#[derive(Clone, Debug)]
pub struct CharacterArt {
    pub info: &'static CharacterInfo,
    pub idle: Animation,
    pub walk: Animation,
    pub attack: Animation,
    pub hurt: Animation,
    pub death: Animation,
    /// Row 6: see [`CharacterInfo::special`].
    pub special: Animation,
}

impl CharacterArt {
    /// All six animations with their names, in sheet order.
    pub fn animations(&self) -> [(&'static str, &Animation); 6] {
        let special = match self.info.special {
            Special::Attack2 => "attack 2",
            Special::Shoot => "shoot",
            Special::Cast => "cast",
            Special::Work => "work",
        };
        [
            ("idle", &self.idle),
            ("walk", &self.walk),
            ("attack", &self.attack),
            ("hurt", &self.hurt),
            ("death", &self.death),
            (special, &self.special),
        ]
    }
}

/// A building's looping idle animation and its three states.
#[derive(Clone, Debug)]
pub struct StructureArt {
    pub info: &'static StructureInfo,
    pub idle: Animation,
    pub construction: ImageId,
    pub damaged: ImageId,
    pub ruined: ImageId,
}

/// Fire effects: flames loop at 10 fps, smoke and embers play once.
#[derive(Clone, Debug)]
pub struct FireArt {
    pub small: Animation,
    pub big: Animation,
    pub smoke: Animation,
    pub embers: Animation,
    /// Flat ground decals (draw with `WorldDraw::decal` or as a flat sprite).
    pub scorch: ImageId,
    pub rubble: ImageId,
}

/// A terrain's images: top variants (animated in place for water and lava) and cliff tiles.
#[derive(Clone, Debug)]
pub struct TerrainArt {
    pub tops: Vec<ImageId>,
    pub cliffs: Vec<ImageId>,
}

/// Loads pack sheets into [`Assets`] on demand and caches them.
pub struct Pack {
    /// Every image is enlarged by this (nearest neighbour) when loaded; see [`Pack::with_upscale`].
    upscale: u32,
    buildings: HashMap<(&'static str, [u8; 4]), ImageId>,
    characters: HashMap<(&'static str, [u8; 4]), CharacterArt>,
    structures: HashMap<(&'static str, [u8; 4]), StructureArt>,
    terrain: HashMap<&'static str, TerrainArt>,
    props: HashMap<&'static str, Vec<ImageId>>,
    fire: Option<FireArt>,
}

impl Default for Pack {
    fn default() -> Self {
        Pack {
            upscale: 1,
            buildings: HashMap::new(),
            characters: HashMap::new(),
            structures: HashMap::new(),
            terrain: HashMap::new(),
            props: HashMap::new(),
            fire: None,
        }
    }
}

/// Enlarge by `k` (no-op for 1).
fn up(img: Image, k: u32) -> Image {
    if k > 1 { img.upscale(k) } else { img }
}

fn decode(bytes: &[u8]) -> Image {
    Image::from_png(bytes).expect("bundled pack png")
}

fn key(team: Option<Color>) -> [u8; 4] {
    team.unwrap_or(NEUTRAL_TEAM).to_rgba8()
}

/// Opaque bounds `(x0, y0, x1, y1)` of a region of `img`, or `None` if it's empty.
fn bounds(img: &Image, x: u32, y: u32, w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for yy in 0..h {
        for xx in 0..w {
            if img.get(x + xx, y + yy)[3] > 0 {
                (x0, y0, x1, y1) = (x0.min(xx), y0.min(yy), x1.max(xx + 1), y1.max(yy + 1));
            }
        }
    }
    (x1 > x0).then_some((x0, y0, x1, y1))
}

/// Cut a grid of cells into frames that share one size: empty columns are trimmed evenly on
/// both sides (keeping the cell's centre) and empty rows only at the top (keeping the baseline,
/// so flyers keep hovering). Returns frames per requested `(row, count)`.
fn cut_cells(img: &Image, cw: u32, ch: u32, rows: &[(u32, u32)]) -> Vec<Vec<Image>> {
    let (mut side, mut top) = (cw / 2, ch);
    for &(row, count) in rows {
        for col in 0..count {
            if let Some((x0, y0, x1, _)) = bounds(img, col * cw, row * ch, cw, ch) {
                side = side.min(x0).min(cw - x1);
                top = top.min(y0);
            }
        }
    }
    let top = top.min(ch - 1);
    rows.iter()
        .map(|&(row, count)| {
            (0..count).map(|col| img.crop(col * cw + side, row * ch + top, cw - 2 * side, ch - top)).collect()
        })
        .collect()
}

impl Pack {
    pub fn new() -> Self {
        Pack::default()
    }

    /// Enlarge every image `k` times when loading (nearest neighbour), for games whose other art
    /// is already drawn at `k` × 16 px per unit: pack sprites then need no scale. It costs `k²`
    /// atlas space, so prefer [`Pack::scale`] when starting fresh.
    pub fn with_upscale(k: u32) -> Self {
        Pack { upscale: k.max(1), ..Pack::default() }
    }

    /// Sprite scale for pack art in `world` (its `pixels_per_unit` over the pack's art pixels).
    pub fn scale(&self, world: &HexWorld) -> f32 {
        world.pixels_per_unit / (ART_PIXELS_PER_UNIT * self.upscale as f32)
    }

    pub fn character_info(id: &str) -> Option<&'static CharacterInfo> {
        CHARACTERS.iter().find(|c| c.id == id)
    }

    pub fn structure_info(id: &str) -> Option<&'static StructureInfo> {
        STRUCTURES.iter().find(|c| c.id == id)
    }

    /// A unit, hero or monster (`id` like `unit_human_footman`, see [`CHARACTERS`]) with its
    /// team-coloured parts in `team` (or [`NEUTRAL_TEAM`]). Panics on an unknown id.
    pub fn character(&mut self, assets: &mut Assets, id: &str, team: Option<Color>) -> CharacterArt {
        let info = Self::character_info(id).unwrap_or_else(|| panic!("no character {id:?} in the pack"));
        let k = self.upscale;
        self.characters
            .entry((info.id, key(team)))
            .or_insert_with(|| {
                let sheet = decode(info.png).recolor_magenta(team.unwrap_or(NEUTRAL_TEAM));
                let special_frames = if info.special == Special::Work { 4 } else { 6 };
                let rows = [(0, 4), (1, 6), (2, 6), (3, 2), (4, 6), (5, special_frames)];
                let mut cut = cut_cells(&sheet, info.cell, info.cell, &rows).into_iter();
                let mut anim = |fps, looping| {
                    let frames = cut.next().unwrap().into_iter().map(|f| assets.add(up(f, k))).collect();
                    Animation::new(frames, fps, looping)
                };
                CharacterArt {
                    info,
                    idle: anim(5.0, true),
                    walk: anim(10.0, true),
                    attack: anim(12.0, false),
                    hurt: anim(8.0, false),
                    death: anim(8.0, false),
                    special: anim(
                        if info.special == Special::Work { 8.0 } else { 12.0 },
                        info.special == Special::Work,
                    ),
                }
            })
            .clone()
    }

    /// A building's frames: the 4 idle frames, then construction, damaged and ruined.
    fn structure_frames(&self, info: &StructureInfo, team: Option<Color>) -> (Vec<Image>, Vec<Image>) {
        let sheet = decode(info.png).recolor_magenta(team.unwrap_or(NEUTRAL_TEAM));
        let (cw, ch) = info.cell;
        let mut cut = cut_cells(&sheet, cw, ch, &[(0, 4), (1, 3)]).into_iter();
        let k = self.upscale;
        let mut next = || cut.next().unwrap().into_iter().map(|f| up(f, k)).collect::<Vec<_>>();
        (next(), next())
    }

    /// A building as one image whose idle animation (smoke, flags, glow) plays in place in the
    /// atlas: draw it like any static image. Panics on an unknown id.
    pub fn structure_image(&mut self, assets: &mut Assets, id: &str, team: Option<Color>) -> ImageId {
        let info = Self::structure_info(id).unwrap_or_else(|| panic!("no structure {id:?} in the pack"));
        if let Some(&img) = self.buildings.get(&(info.id, key(team))) {
            return img;
        }
        let (idle, _) = self.structure_frames(info, team);
        let img = assets.add_animated(idle, 4.0);
        self.buildings.insert((info.id, key(team)), img);
        img
    }

    /// A building (`id` like `building_human_house`, see [`STRUCTURES`]). Panics on an unknown id.
    pub fn structure(&mut self, assets: &mut Assets, id: &str, team: Option<Color>) -> StructureArt {
        let info = Self::structure_info(id).unwrap_or_else(|| panic!("no structure {id:?} in the pack"));
        if !self.structures.contains_key(&(info.id, key(team))) {
            let (idle, states) = self.structure_frames(info, team);
            let idle = idle.into_iter().map(|f| assets.add(f)).collect();
            let states: Vec<ImageId> = states.into_iter().map(|f| assets.add(f)).collect();
            self.structures.insert(
                (info.id, key(team)),
                StructureArt {
                    info,
                    idle: Animation::new(idle, 4.0, true),
                    construction: states[0],
                    damaged: states[1],
                    ruined: states[2],
                },
            );
        }
        self.structures[&(info.id, key(team))].clone()
    }

    /// A terrain's images by short name (`grass`, `water`, `road_cobble`, ... see [`TERRAIN`]).
    /// Animated terrain (water, lava) gets two top variants that animate in place.
    pub fn terrain(&mut self, assets: &mut Assets, name: &str) -> TerrainArt {
        let info = TERRAIN.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no terrain {name:?}"));
        let k = self.upscale;
        self.terrain
            .entry(info.name)
            .or_insert_with(|| {
                let img = decode(info.png);
                let tile = |col: u32, row_y: u32, h: u32| up(img.crop(col * 32, row_y, 32, h), k);
                if info.animated {
                    let tops = (0..2)
                        .map(|v| assets.add_animated((0..4).map(|f| tile(f, v * 32, 32)).collect(), 4.0))
                        .collect();
                    let cliffs = (0..4).map(|c| assets.add(tile(c, 64, 16))).collect();
                    TerrainArt { tops, cliffs }
                } else {
                    let tops = (0..4).map(|c| assets.add(tile(c, 0, 32))).collect();
                    let cliffs = (0..4).map(|c| assets.add(tile(c, 32, 16))).collect();
                    TerrainArt { tops, cliffs }
                }
            })
            .clone()
    }

    /// A ready-made [`Material`] for a terrain: textured top and cliffs (tiled at true pixel size),
    /// with a sensible movement cost (roads fast, swamp and snow slow, lava not walkable).
    pub fn material(&mut self, assets: &mut Assets, name: &str) -> Material {
        let art = self.terrain(assets, name);
        let mut m = Material::color(name, Color::WHITE, Color::WHITE);
        m.textures = art.tops;
        m.side_texture = art.cliffs.first().copied();
        m.side_texture_size = Some(Vec2::new(32.0, 16.0) / ART_PIXELS_PER_UNIT);
        m.move_cost = match name {
            "road_cobble" => 0.5,
            "road_dirt" => 0.7,
            "forest_floor" | "sand" | "desert" | "ice" | "farmland" => 1.3,
            "snow" | "rock" | "elf_moss" => 1.5,
            "swamp" => 2.0,
            _ => 1.0,
        };
        m.walkable = !matches!(name, "lava" | "water");
        m
    }

    /// The four props of a sheet (`props_trees_oak`, `props_rocks`, ... see [`PROPS`]): usually
    /// three variants and a burnt / dead one last.
    pub fn props(&mut self, assets: &mut Assets, id: &str) -> Vec<ImageId> {
        let info = PROPS.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("no props {id:?}"));
        let k = self.upscale;
        self.props
            .entry(info.id)
            .or_insert_with(|| {
                let img = decode(info.png);
                let (cw, ch) = info.cell;
                // Each prop is trimmed on its own (props don't animate).
                (0..4)
                    .map(|c| {
                        let cell = img.crop(c * cw, 0, cw, ch);
                        let frames = cut_cells(&cell, cw, ch, &[(0, 1)]);
                        assets.add(up(frames.into_iter().next().unwrap().remove(0), k))
                    })
                    .collect()
            })
            .clone()
    }

    /// Flames, smoke, embers and burn decals.
    pub fn fire(&mut self, assets: &mut Assets) -> FireArt {
        let k = self.upscale;
        self.fire
            .get_or_insert_with(|| {
                let img = decode(FX_FIRE);
                let strip = |x0: u32, y: u32, w: u32, h: u32, n: u32| -> Vec<Image> {
                    (0..n).map(|i| up(img.crop(x0 + i * w, y, w, h), k)).collect()
                };
                let mut anim = |frames: Vec<Image>, fps, looping| {
                    Animation::new(frames.into_iter().map(|f| assets.add(f)).collect(), fps, looping)
                };
                let small = anim(strip(0, 0, 16, 24, 8), 10.0, true);
                let big = anim(strip(0, 24, 32, 48, 8), 10.0, true);
                let smoke = anim(strip(0, 72, 16, 16, 4), 6.0, false);
                let embers = anim(strip(64, 72, 16, 16, 4), 8.0, false);
                FireArt {
                    small,
                    big,
                    smoke,
                    embers,
                    scorch: assets.add(up(img.crop(0, 88, 32, 16), k)),
                    rubble: assets.add(up(img.crop(32, 88, 32, 16), k)),
                }
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_matches_the_files() {
        for c in CHARACTERS {
            let img = decode(c.png);
            assert_eq!((img.width, img.height), (c.cell * 6, c.cell * 6), "{}", c.id);
        }
        for s in STRUCTURES {
            let img = decode(s.png);
            assert_eq!((img.width, img.height), (s.cell.0 * 4, s.cell.1 * 2), "{}", s.id);
        }
        for name in ["road_cobble", "road_dirt", "forest_floor", "elf_moss", "lava", "water", "swamp"] {
            assert!(TERRAIN.iter().any(|t| t.name == name), "{name}");
        }
        for t in TERRAIN {
            let img = decode(t.png);
            assert_eq!((img.width, img.height), (128, if t.animated { 80 } else { 48 }), "{}", t.id);
        }
        for p in PROPS {
            let img = decode(p.png);
            assert_eq!((img.width, img.height), (p.cell.0 * 4, p.cell.1), "{}", p.id);
        }
    }

    #[test]
    fn loads_everything_with_shared_anchors() {
        let mut assets = Assets::new();
        let mut pack = Pack::new();
        for c in CHARACTERS {
            let art = pack.character(&mut assets, c.id, Some(Color::hex(0x3d7be0)));
            let first = assets.region(art.idle.frames[0]);
            for (name, a) in art.animations() {
                assert!(!a.frames.is_empty(), "{} {name}", c.id);
                for &f in &a.frames {
                    let r = assets.region(f);
                    assert_eq!((r.width, r.height), (first.width, first.height), "{} {name}", c.id);
                }
            }
            // Ground units stand on the bottom row; the trim never cuts the baseline.
            assert!(first.height <= c.cell && first.width <= c.cell && first.width % 2 == c.cell % 2);
        }
        for s in STRUCTURES {
            pack.structure(&mut assets, s.id, None);
        }
        for t in TERRAIN {
            let m = pack.material(&mut assets, t.name);
            assert_eq!(m.textures.len(), if t.animated { 2 } else { 4 });
        }
        for p in PROPS {
            assert_eq!(pack.props(&mut assets, p.id).len(), 4);
        }
        pack.fire(&mut assets);
        // Cached: asking again adds nothing; another team colour is a new copy.
        let used = assets.atlas().usage();
        pack.character(&mut assets, "unit_human_footman", Some(Color::hex(0x3d7be0)));
        assert_eq!(assets.atlas().usage(), used);
        assert!(assets.atlas().page_count() >= 1);
    }

    #[test]
    fn upscaled_pack_and_animated_buildings() {
        let mut assets = Assets::new();
        let (mut one, mut two) = (Pack::new(), Pack::with_upscale(2));
        let a = one.character(&mut assets, "unit_human_footman", None);
        let b = two.character(&mut assets, "unit_human_footman", None);
        let (ra, rb) = (assets.region(a.idle.frames[0]), assets.region(b.idle.frames[0]));
        assert_eq!((rb.width, rb.height), (ra.width * 2, ra.height * 2));
        let world = HexWorld::new(knight_core::hex::Layout::pointy(1.0));
        assert_eq!(one.scale(&world), 2.0 * two.scale(&world));
        let house = two.structure_image(&mut assets, "building_human_house", None);
        assert_eq!(two.structure_image(&mut assets, "building_human_house", None), house, "cached");
        let v = assets.atlas_version();
        assets.update(10.3);
        assert!(assets.atlas_version() > v, "the idle animation plays in place");
    }

    #[test]
    fn team_colour_replaces_magenta() {
        let mut assets = Assets::new();
        let mut pack = Pack::new();
        let red = Color::hex(0xff0000);
        let art = pack.character(&mut assets, "unit_human_footman", Some(red));
        let r = assets.region(art.idle.frames[0]);
        let mut has_team = false;
        for y in r.y..r.y + r.height {
            for x in r.x..r.x + r.width {
                let p = assets.atlas().pixel(r.page, x, y);
                assert!(!(p[3] > 0 && p[1] == 0 && p[0] == p[2] && p[0] >= 0x66), "magenta left at {x},{y}");
                has_team |= p[3] > 0 && p[0] > 90 && p[1] == 0 && p[2] == 0;
            }
        }
        assert!(has_team, "team colour applied");
    }
}
