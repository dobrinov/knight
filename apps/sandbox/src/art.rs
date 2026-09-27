//! Sandbox art: the `knight-assets` pack (terrain, trees, units, buildings) plus a few
//! placeholders the pack doesn't cover yet (icons, emotes, projectiles, flags, trains).

use std::collections::HashMap;
use std::rc::Rc;

use knight_assets::{Pack, Special};
use knight_engine::*;

macro_rules! png {
    ($path:expr) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/", $path))
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Team {
    Blue,
    Red,
}

impl Team {
    pub fn color(self) -> Color {
        match self {
            Team::Blue => Color::hex(0x3d7be0),
            Team::Red => Color::hex(0xd8453c),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnitKind {
    Griffon,
    Archer,
    Knight,
    Ogre,
    Paladin,
    Footman,
    Wolf,
}

impl UnitKind {
    pub const ALL: [UnitKind; 7] = [
        UnitKind::Griffon,
        UnitKind::Archer,
        UnitKind::Knight,
        UnitKind::Ogre,
        UnitKind::Paladin,
        UnitKind::Footman,
        UnitKind::Wolf,
    ];

    /// The pack character drawn for this kind.
    pub fn pack_id(self) -> &'static str {
        match self {
            UnitKind::Griffon => "unit_human_griffon",
            UnitKind::Archer => "unit_human_archer",
            UnitKind::Knight => "unit_human_knight",
            UnitKind::Ogre => "unit_orc_ogre",
            UnitKind::Paladin => "hero_human_paladin",
            UnitKind::Footman => "unit_human_footman",
            UnitKind::Wolf => "monster_wolf",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            UnitKind::Griffon => "Griffon Rider",
            UnitKind::Archer => "Archer",
            UnitKind::Knight => "Knight",
            UnitKind::Ogre => "Ogre",
            UnitKind::Paladin => "Paladin",
            UnitKind::Footman => "Footman",
            UnitKind::Wolf => "Wolf",
        }
    }
}

/// Animations of one unit type in one team colour.
#[derive(Clone, Debug)]
pub struct UnitArt {
    pub idle: Animation,
    pub walk: Animation,
    pub attack: Animation,
    pub hurt: Animation,
    pub death: Animation,
    pub shoot: Option<Animation>,
}

#[derive(Clone, Copy, Debug)]
pub struct Mats {
    pub seabed: MaterialId,
    pub sand: MaterialId,
    pub grass: MaterialId,
    pub meadow: MaterialId,
    pub forest: MaterialId,
    pub dirt: MaterialId,
    pub rock: MaterialId,
    pub snow: MaterialId,
    pub road: MaterialId,
}

pub struct Art {
    units: HashMap<(UnitKind, Team), UnitArt>,
    pub keep: [ImageId; 2],
    pub keep_neutral: ImageId,
    pub village: ImageId,
    pub village_neutral: ImageId,
    pub sawmill: ImageId,
    pub ore_mine: ImageId,
    pub treasure: ImageId,
    pub shrine: ImageId,
    pub trees: [ImageId; 2],
    pub gold: ImageId,
    pub wood: ImageId,
    pub ore: ImageId,
    pub bolt: ImageId,
    pub arrow: ImageId,
    pub locomotive: ImageId,
    pub wagon: ImageId,
    pub rail: ImageId,
    pub flag: [ImageId; 2],
    /// Emote icons: (command name, image).
    pub emotes: Vec<(&'static str, ImageId)>,
    /// Sound effects and music.
    pub sfx: Sounds,
    /// Animated water texture.
    pub water: ImageId,
    /// Pack terrain materials by sandbox name.
    mats: HashMap<&'static str, Material>,
}

/// The sandbox's sounds, all synthesized at startup (no audio files needed).
#[derive(Clone, Copy, Debug)]
pub struct Sounds {
    pub click: SoundId,
    pub coin: SoundId,
    pub hit: SoundId,
    pub swing: SoundId,
    pub shoot: SoundId,
    pub explosion: SoundId,
    pub magic: SoundId,
    pub step: SoundId,
    pub pop: SoundId,
    pub powerup: SoundId,
    pub deny: SoundId,
    pub music: SoundId,
    pub battle_music: SoundId,
}

impl Sounds {
    fn new(audio: &mut Audio) -> Sounds {
        let mut add = |sfx: Sfx, seed| audio.add(sfx.render(seed));
        let s = Sounds {
            click: add(Sfx::click(), 1),
            coin: add(Sfx::coin(), 2),
            hit: add(Sfx::hit(), 3),
            swing: add(Sfx::swing(), 4),
            shoot: add(Sfx::shoot(), 5),
            explosion: add(Sfx::explosion(), 6),
            magic: add(Sfx::magic(), 7),
            step: add(Sfx::step(), 8),
            pop: add(Sfx::pop(), 9),
            powerup: add(Sfx::powerup(), 10),
            deny: add(Sfx::deny(), 11),
            music: audio.add(knight_engine::synth::chiptune(7, 112.0, 8)),
            battle_music: audio.add(knight_engine::synth::chiptune(21, 140.0, 8)),
        };
        // Every UI button click plays this (the engine does it).
        audio.ui_click = Some(s.click);
        s
    }
}

fn load(bytes: &[u8]) -> Image {
    Image::from_png(bytes).expect("bundled png")
}

/// A 12×12 pixel icon from a character grid ('.' clear, 'o' outline, 'a' main, 'b' light,
/// 'c' detail), doubled like the rest of the art.
fn icon(rows: [&str; 12], main: u32, light: u32, detail: u32) -> Image {
    Image::from_fn(12, 12, |x, y| {
        let ch = rows[y as usize].as_bytes().get(x as usize).copied().unwrap_or(b'.');
        match ch {
            b'o' => [27, 20, 32, 255],
            b'a' => Color::hex(main).to_rgba8(),
            b'b' => Color::hex(light).to_rgba8(),
            b'c' => Color::hex(detail).to_rgba8(),
            _ => [0, 0, 0, 0],
        }
    })
    .upscale(2)
}

fn emote_icons() -> Vec<(&'static str, Image)> {
    const FACE: [&str; 12] = [
        "...oooooo...",
        "..oaaaaaao..",
        ".oaaaaaaaao.",
        "oaaccaaccaao",
        "oaaccaaccaao",
        "oaaaaaaaaaao",
        "oacaaaaaacao",
        "oaacaaaacaao",
        ".oaaccccaao.",
        "..oaaaaaao..",
        "...oooooo...",
        "............",
    ];
    vec![
        (
            "love",
            icon(
                [
                    "..oo....oo..",
                    ".oaao..oaao.",
                    "oabaaooaaaao",
                    "oaaaaaaaaaao",
                    "oaaaaaaaaaao",
                    ".oaaaaaaaao.",
                    "..oaaaaaao..",
                    "...oaaaao...",
                    "....oaao....",
                    ".....oo.....",
                    "............",
                    "............",
                ],
                0xe8454c,
                0xff9aa0,
                0,
            ),
        ),
        ("smile", icon(FACE, 0xf6c945, 0xfff0a0, 0x3a2a10)),
        (
            "laugh",
            icon(
                [
                    "...oooooo...",
                    "..oaaaaaao..",
                    ".oaaaaaaaao.",
                    "oacacaacacao",
                    "oaaaaaaaaaao",
                    "oaccccccccao",
                    "oacbbbbbbcao",
                    "oaaccccccaao",
                    ".oaaaaaaaao.",
                    "..oaaaaaao..",
                    "...oooooo...",
                    "............",
                ],
                0xf6c945,
                0xe8454c,
                0x3a2a10,
            ),
        ),
        (
            "wow",
            icon(
                [
                    ".....oo.....",
                    "....oaao....",
                    "....oaao....",
                    "....oaao....",
                    "....oaao....",
                    "....oaao....",
                    ".....oo.....",
                    "............",
                    ".....oo.....",
                    "....oaao....",
                    ".....oo.....",
                    "............",
                ],
                0xffd84a,
                0xfff0a0,
                0,
            ),
        ),
        (
            "what",
            icon(
                [
                    "...oooooo...",
                    "..oaaaaaao..",
                    "..oaooooao..",
                    "...o...oao..",
                    ".......oao..",
                    "......oao...",
                    ".....oao....",
                    ".....oao....",
                    "......o.....",
                    ".....oo.....",
                    "....oaao....",
                    ".....oo.....",
                ],
                0x9fd8ff,
                0xd0f0ff,
                0,
            ),
        ),
        (
            "angry",
            icon(
                [
                    "...oooooo...",
                    "..oaaaaaao..",
                    ".occaaaacco.",
                    "oaaccaaccaao",
                    "oaaacaacaaao",
                    "oaaaaaaaaaao",
                    "oaaaaaaaaaao",
                    "oaaccccccaao",
                    ".oacaaaacao.",
                    "..oaaaaaao..",
                    "...oooooo...",
                    "............",
                ],
                0xe06040,
                0xff9a70,
                0x301010,
            ),
        ),
        (
            "sleep",
            icon(
                [
                    "......ooooo.",
                    "......oaaao.",
                    "........oao.",
                    ".......oao..",
                    "......oaaao.",
                    "ooooo.ooooo.",
                    "oaaao.......",
                    "..oao.......",
                    ".oao........",
                    "oaaao.......",
                    "ooooo.......",
                    "............",
                ],
                0xbcd4ff,
                0xe0ecff,
                0,
            ),
        ),
        (
            "cheer",
            icon(
                [
                    ".....oo.....",
                    ".....oao....",
                    "....oaao....",
                    "ooooaaaoooo.",
                    "oaaaaaaaaao.",
                    ".oaaaabaao..",
                    "..oaaaaao...",
                    "..oaaoaao...",
                    ".oaao.oaao..",
                    ".oao...oao..",
                    ".oo.....oo..",
                    "............",
                ],
                0xffd84a,
                0xfff4b0,
                0,
            ),
        ),
    ]
}

/// Paint rectangles into an image (for small procedural sprites).
fn paint(w: u32, h: u32, rects: &[(u32, u32, u32, u32, u32)]) -> Image {
    let mut img = Image::new(w, h);
    for &(x, y, rw, rh, c) in rects {
        let col = if c == 0 { [0, 0, 0, 0] } else { Color::hex(c).to_rgba8() };
        for yy in y..(y + rh).min(h) {
            for xx in x..(x + rw).min(w) {
                img.set(xx, yy, col);
            }
        }
    }
    img
}

fn outline(img: &Image) -> Image {
    let mut out = img.clone();
    for y in 0..img.height {
        for x in 0..img.width {
            if img.get(x, y)[3] != 0 {
                continue;
            }
            let near = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                nx >= 0
                    && ny >= 0
                    && nx < img.width as i32
                    && ny < img.height as i32
                    && img.get(nx as u32, ny as u32)[3] != 0
            });
            if near {
                out.set(x, y, [27, 20, 32, 255]);
            }
        }
    }
    out
}

impl Art {
    pub fn load(ctx: &mut Context) -> Rc<Art> {
        let sfx = Sounds::new(&mut ctx.audio);
        let assets = &mut ctx.assets;
        // The pack is drawn at 16 px per unit; the sandbox's other art is doubled, so double the
        // pack too and every sprite shares one pixel grid (and pixel-art zoom levels).
        let mut pack = Pack::with_upscale(2);
        let mut units = HashMap::new();
        for kind in UnitKind::ALL {
            for team in [Team::Blue, Team::Red] {
                let c = pack.character(assets, kind.pack_id(), Some(team.color()));
                let shoot = (c.info.special == Special::Shoot).then(|| c.special.clone());
                units.insert(
                    (kind, team),
                    UnitArt { idle: c.idle, walk: c.walk, attack: c.attack, hurt: c.hurt, death: c.death, shoot },
                );
            }
        }
        let mut building = |id: &str, team: Option<Team>| pack.structure_image(assets, id, team.map(Team::color));
        let keep =
            [building("building_human_castle", Some(Team::Blue)), building("building_human_castle", Some(Team::Red))];
        let keep_neutral = building("building_human_castle", None);
        let village = building("building_neutral_village", Some(Team::Blue));
        let village_neutral = building("building_neutral_village", None);
        let sawmill = building("building_neutral_sawmill", None);
        let ore_mine = building("building_neutral_quarry", None);
        let treasure = building("building_neutral_chest", None);
        let shrine = building("building_neutral_shrine", None);
        let pines = pack.props(assets, "props_trees_pine");
        let trees = [pines[0], pines[2]];
        let water = pack.terrain(assets, "water").tops[0];
        let mut mats = HashMap::new();
        for (name, terrain) in [
            ("seabed", "seabed"),
            ("sand", "sand"),
            ("grass", "grass"),
            ("meadow", "meadow"),
            ("forest", "forest_floor"),
            ("dirt", "dirt"),
            ("rock", "rock"),
            ("snow", "snow"),
            ("road", "road_cobble"),
        ] {
            mats.insert(name, pack.material(assets, terrain));
        }

        let locomotive = assets.add(outline(&paint(
            40,
            26,
            &[
                (6, 4, 6, 6, 0x303038),
                (4, 2, 10, 3, 0x505060),
                (2, 10, 26, 10, 0x2f6fc0),
                (26, 4, 12, 16, 0xa03030),
                (28, 6, 8, 5, 0xe8e0a0),
                (2, 12, 26, 2, 0xd8b040),
                (0, 19, 40, 2, 0x303038),
                (4, 20, 6, 6, 0x202028),
                (14, 20, 6, 6, 0x202028),
                (28, 20, 8, 6, 0x202028),
                (6, 22, 2, 2, 0x9090a0),
                (16, 22, 2, 2, 0x9090a0),
                (31, 22, 2, 2, 0x9090a0),
            ],
        )));
        let wagon = assets.add(outline(&paint(
            32,
            22,
            &[
                (1, 6, 30, 10, 0x8a5a30),
                (1, 6, 30, 2, 0xb07a44),
                (4, 2, 24, 5, 0xe0c060),
                (0, 15, 32, 2, 0x303038),
                (4, 16, 6, 6, 0x202028),
                (22, 16, 6, 6, 0x202028),
            ],
        )));
        let rail = assets.add(Image::from_fn(16, 16, |x, y| {
            if y == 3 || y == 12 {
                [180, 180, 196, 255]
            } else if y == 4 || y == 13 {
                [90, 90, 104, 255]
            } else if x % 5 < 2 {
                [104, 72, 44, 255]
            } else {
                [0, 0, 0, 0]
            }
        }));
        let flag = [Team::Blue, Team::Red].map(|t| {
            let c = t.color().to_rgba8();
            assets.add(outline(&Image::from_fn(16, 28, |x, y| {
                if x == 2 {
                    [120, 96, 70, 255]
                } else if (3..14).contains(&x) && y < 10 && y as i32 >= (x as i32 - 3) / 3 {
                    c
                } else {
                    [0, 0, 0, 0]
                }
            })))
        });

        Rc::new(Art {
            units,
            keep,
            keep_neutral,
            village,
            village_neutral,
            sawmill,
            ore_mine,
            treasure,
            shrine,
            trees,
            gold: assets.add(load(png!("icons/gold.png"))),
            wood: assets.add(load(png!("icons/wood.png"))),
            ore: assets.add(load(png!("icons/ore.png"))),
            bolt: assets.add(load(png!("icons/lightning_bolt.png"))),
            arrow: assets.add(load(png!("fx/arrow.png"))),
            locomotive,
            wagon,
            rail,
            flag,
            emotes: emote_icons().into_iter().map(|(n, img)| (n, assets.add(img))).collect(),
            sfx,
            water,
            mats,
        })
    }

    pub fn unit(&self, kind: UnitKind, team: Team) -> &UnitArt {
        &self.units[&(kind, team)]
    }

    /// Register the standard terrain materials on a world.
    pub fn materials(&self, world: &mut HexWorld) -> Mats {
        // Movement cost per terrain: roads are twice as fast, forests and snow slow you down.
        let mut add = |name: &'static str, move_cost: f32| {
            let mut m = self.mats[name].clone();
            m.name = name.to_string();
            m.move_cost = move_cost;
            world.add_material(m)
        };
        Mats {
            seabed: add("seabed", 1.0),
            sand: add("sand", 1.5),
            grass: add("grass", 1.0),
            meadow: add("meadow", 1.0),
            forest: add("forest", 2.0),
            dirt: add("dirt", 1.25),
            rock: add("rock", 1.5),
            snow: add("snow", 2.5),
            road: add("road", 0.5),
        }
    }
}
