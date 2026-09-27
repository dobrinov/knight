//! Asset Pack: every sheet of `knight-assets` on a hex map. Terrain patches show top variants,
//! cliffs tiled at true pixel size, animated water and lava, and props; the character and
//! building tabs show every animation and state in any team colour.

use std::rc::Rc;

use knight_assets::{CHARACTERS, CharacterArt, Faction, Kind, Pack, STRUCTURES, StructureArt, TERRAIN};
use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::{Hex, Rng, shapes};
use knight_engine::*;

use crate::art::Art;
use crate::common;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Terrain,
    Units,
    Heroes,
    Monsters,
    Buildings,
}

const TABS: [(Tab, &str); 5] = [
    (Tab::Terrain, "Terrain"),
    (Tab::Units, "Units"),
    (Tab::Heroes, "Heroes"),
    (Tab::Monsters, "Monsters"),
    (Tab::Buildings, "Buildings"),
];

const TEAMS: [(&str, Option<u32>); 3] = [("Blue", Some(0x3d7be0)), ("Red", Some(0xd8453c)), ("Neutral", None)];
const STATES: [&str; 4] = ["Normal", "Building", "Damaged", "Ruined"];

/// Something standing on the map.
enum Thing {
    Prop(ImageId),
    Fire,
    Character(CharacterArt),
    Structure(StructureArt),
}

struct Placed {
    hex: Hex,
    offset: Vec2,
    thing: Thing,
    label: Option<String>,
}

pub struct AssetPackDemo {
    world: HexWorld,
    camera: Camera,
    controller: CameraController,
    pack: Pack,
    tab: Tab,
    team: usize,
    anim: usize,
    state: usize,
    things: Vec<Placed>,
    /// Patch labels on the terrain tab: (world position, name).
    labels: Vec<(Vec3, String)>,
    fire: Option<knight_assets::FireArt>,
    dirty: bool,
    time: f32,
}

impl AssetPackDemo {
    pub fn new(_art: Rc<Art>) -> Self {
        let world = HexWorld::new(hex::Layout::pointy(1.0));
        AssetPackDemo {
            camera: Camera::new(Vec3::ZERO, 40.0).with_pitch(55.0),
            world,
            controller: CameraController::default(),
            pack: Pack::new(),
            tab: Tab::Terrain,
            team: 0,
            anim: 1,
            state: 0,
            things: Vec::new(),
            labels: Vec::new(),
            fire: None,
            dirty: true,
            time: 0.0,
        }
    }

    fn team(&self) -> Option<Color> {
        TEAMS[self.team].1.map(Color::hex)
    }

    fn rebuild(&mut self, assets: &mut Assets) {
        self.things.clear();
        self.labels.clear();
        self.world.clear();
        self.world.materials.clear();
        self.world.water = None;
        self.fire = Some(self.pack.fire(assets));
        match self.tab {
            Tab::Terrain => self.build_terrain(assets),
            Tab::Units | Tab::Heroes | Tab::Monsters => self.build_characters(assets),
            Tab::Buildings => self.build_structures(assets),
        }
        self.world.mark_all_dirty();
        // Look at the middle of what's on show (rows can be shorter than the lawn).
        let c = if self.tab == Tab::Terrain || self.things.is_empty() {
            self.world.center()
        } else {
            let sum: Vec3 = self.things.iter().map(|p| self.world.hex_to_world(p.hex)).sum();
            sum / self.things.len() as f32
        };
        self.camera.target = Vec3::new(c.x, 0.0, c.z);
    }

    /// A lawn of pack grass for the character and building tabs.
    fn lawn(&mut self, assets: &mut Assets, w: i32, h: i32) {
        let grass = self.world.add_material(self.pack.material(assets, "grass"));
        let dirt = self.world.add_material(self.pack.material(assets, "road_dirt"));
        for hx in shapes::rectangle(w, h, true, hex::Parity::Odd) {
            let o = hex::Offset::from_hex_r(hx, hex::Parity::Odd);
            let path = o.row % 3 == 2;
            self.world.set_tile(hx, Tile::new(1, if path { dirt } else { grass }));
        }
    }

    fn build_terrain(&mut self, assets: &mut Assets) {
        let water = self.pack.terrain(assets, "water");
        let seabed = self.world.add_material(self.pack.material(assets, "seabed"));
        self.world.water = Some(Water { level: 0, color: Color::WHITE, texture: Some(water.tops[0]) });
        let names: Vec<&str> = TERRAIN.iter().map(|t| t.name).filter(|n| *n != "water" && *n != "seabed").collect();
        let cols = 6;
        let mut rng = Rng::new(3);
        // Sea everywhere, then one raised patch per terrain.
        let rows = names.len().div_ceil(cols) as i32;
        for hx in shapes::rectangle(cols as i32 * 7 + 3, rows * 6 + 3, true, hex::Parity::Odd) {
            self.world.set_tile(hx, Tile::new(-1, seabed));
        }
        for (i, name) in names.iter().enumerate() {
            let m = self.world.add_material(self.pack.material(assets, name));
            let o = hex::Offset::new(4 + (i % cols) as i32 * 7, 3 + (i / cols) as i32 * 6);
            let centre = o.to_hex_r(hex::Parity::Odd);
            let base = 1 + (i % 3) as i16;
            for hx in shapes::hexagon(centre, 2) {
                // A raised middle shows cliffs within the patch too.
                let h = if hx.distance(centre) == 0 {
                    base + 2
                } else if hx.distance(centre) == 1 {
                    base + 1
                } else {
                    base
                };
                self.world.set_tile(hx, Tile::new(h, m));
            }
            let p = self.world.hex_to_world(centre + Hex::new(0, 3));
            self.labels.push((p, name.replace('_', " ")));
            // Props that belong on this ground.
            let props: &[&str] = match *name {
                "grass" => &["props_trees_oak", "props_bushes_flowers"],
                "meadow" => &["props_trees_birch", "props_bushes_flowers"],
                "forest_floor" => &["props_trees_pine"],
                "snow" | "ice" => &["props_trees_snow"],
                "sand" | "desert" => &["props_trees_palm"],
                "elf_moss" => &["props_trees_elf"],
                "ash" | "orc_wasteland" => &["props_trees_dead"],
                "rock" => &["props_mountains", "props_rocks"],
                "swamp" => &["props_bushes_flowers", "props_trees_dead"],
                _ => &[],
            };
            let ring: Vec<Hex> = centre.ring(2);
            for (k, id) in props.iter().enumerate() {
                let imgs = self.pack.props(assets, id);
                let n = if *id == "props_mountains" { 1 } else { 3 };
                for j in 0..n {
                    let hex = if *id == "props_mountains" { centre } else { ring[(k * 5 + j * 4) % ring.len()] };
                    let img = imgs[rng.range(0, 4) as usize];
                    let offset = Vec2::new(rng.range_f32(-0.2, 0.2), rng.range_f32(-0.1, 0.1));
                    self.things.push(Placed { hex, offset, thing: Thing::Prop(img), label: None });
                }
            }
            if *name == "ash" || *name == "lava" {
                self.things.push(Placed { hex: ring[1], offset: Vec2::ZERO, thing: Thing::Fire, label: None });
            }
        }
    }

    fn build_characters(&mut self, assets: &mut Assets) {
        let kind = match self.tab {
            Tab::Units => Kind::Unit,
            Tab::Heroes => Kind::Hero,
            _ => Kind::Monster,
        };
        let list: Vec<_> = CHARACTERS.iter().filter(|c| c.kind == kind).collect();
        let factions = [Faction::Human, Faction::Orc, Faction::Elf, Faction::Monster];
        let per_row = 7;
        let rows: Vec<Vec<_>> = factions
            .iter()
            .flat_map(|f| {
                let of: Vec<_> = list.iter().filter(|c| c.faction == *f).copied().collect();
                of.chunks(per_row).map(|c| c.to_vec()).collect::<Vec<_>>()
            })
            .collect();
        self.lawn(assets, per_row as i32 * 4 + 2, rows.len() as i32 * 3 + 2);
        let team = self.team();
        for (r, row) in rows.iter().enumerate() {
            for (c, info) in row.iter().enumerate() {
                let hex = hex::Offset::new(2 + c as i32 * 4, 2 + r as i32 * 3).to_hex_r(hex::Parity::Odd);
                let art = self.pack.character(assets, info.id, team);
                self.things.push(Placed {
                    hex,
                    offset: Vec2::ZERO,
                    thing: Thing::Character(art),
                    label: Some(info.name.to_string()),
                });
            }
        }
    }

    fn build_structures(&mut self, assets: &mut Assets) {
        let factions = [Faction::Human, Faction::Orc, Faction::Elf, Faction::Neutral];
        let per_row = 6;
        let mut rows: Vec<Vec<_>> = Vec::new();
        for f in factions {
            let of: Vec<_> = STRUCTURES.iter().filter(|s| s.faction == f).collect();
            rows.extend(of.chunks(per_row).map(|c| c.to_vec()));
        }
        self.lawn(assets, per_row as i32 * 5 + 2, rows.len() as i32 * 4 + 2);
        let team = self.team();
        for (r, row) in rows.iter().enumerate() {
            for (c, info) in row.iter().enumerate() {
                let hex = hex::Offset::new(3 + c as i32 * 5, 3 + r as i32 * 4).to_hex_r(hex::Parity::Odd);
                let art = self.pack.structure(assets, info.id, team);
                self.things.push(Placed {
                    hex,
                    offset: Vec2::ZERO,
                    thing: Thing::Structure(art),
                    label: Some(info.name.to_string()),
                });
            }
        }
    }
}

impl Scene for AssetPackDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        for (i, (tab, name)) in TABS.iter().enumerate() {
            if (ctx.input.ui_clicked(name) || ctx.input.key_pressed(DIGITS[i])) && self.tab != *tab {
                self.tab = *tab;
                self.dirty = true;
            }
        }
        for (i, (name, _)) in TEAMS.iter().enumerate() {
            if ctx.input.ui_clicked(name) && self.team != i {
                self.team = i;
                self.dirty = self.tab != Tab::Terrain;
            }
        }
        for i in 0..6 {
            if ctx.input.ui_clicked(&format!("anim{i}")) {
                self.anim = i;
                self.time = 0.0;
            }
        }
        for (i, s) in STATES.iter().enumerate() {
            if ctx.input.ui_clicked(s) {
                self.state = i;
            }
        }
        if self.dirty {
            self.dirty = false;
            self.rebuild(&mut ctx.assets);
        }
        self.time += ctx.time.dt;
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, ctx.time.dt, over_ui);
        ctx.status = format!(
            "asset_pack tab={} things={} pages={:.2} anim={} team={}",
            TABS.iter().find(|t| t.0 == self.tab).unwrap().1,
            self.things.len(),
            ctx.assets.atlas().usage(),
            self.anim,
            TEAMS[self.team].0
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let scale = self.pack.scale(&self.world);
        let (anim, state, time) = (self.anim, self.state, self.time);
        {
            let mut w = f.world(&mut self.world, &self.camera);
            w.background_gradient(Color::hex(0x2a3550), Color::hex(0x121622));
            let t = w.time();
            for p in &self.things {
                let base = w.world.hex_to_world(p.hex);
                let pos = base + Vec3::new(p.offset.x, 0.0, p.offset.y);
                match &p.thing {
                    Thing::Prop(img) => {
                        w.sprite(Sprite::new(*img, pos).scale(scale));
                    }
                    Thing::Fire => {
                        if let Some(fire) = &self.fire {
                            w.sprite(Sprite::new(fire.big.frame_at(t), pos).scale(scale));
                            let e = (t * 0.7).fract() * fire.embers.duration();
                            w.sprite(Sprite::new(fire.embers.frame_at(e), pos + Vec3::Y * 1.6).scale(scale));
                        }
                    }
                    Thing::Character(art) => {
                        let (_, a) = art.animations()[anim];
                        // One-shot animations replay after a short pause.
                        let t = if a.looping { time } else { time % (a.duration() + 0.6) };
                        let r = art.info.cell as f32 / 48.0;
                        w.shadow(pos, 0.45 * r, if art.info.flying { 0.25 } else { 0.35 });
                        w.sprite(Sprite::new(a.frame_at(t), pos).scale(scale));
                    }
                    Thing::Structure(art) => {
                        let img = match state {
                            1 => art.construction,
                            2 => art.damaged,
                            3 => art.ruined,
                            _ => art.idle.frame_at(t),
                        };
                        w.sprite(Sprite::new(img, pos).scale(scale));
                    }
                }
                if let Some(l) = &p.label {
                    w.label(pos + Vec3::new(0.0, 0.0, 0.9), l, 8.0, Color::WHITE);
                }
            }
            for (p, name) in &self.labels {
                w.label(*p, name, 8.0, Color::WHITE);
            }
        }
        common::header(f, "Asset Pack", "knight-assets: terrain, props, fire, units, heroes, monsters, buildings");
        common::stats(f);
        common::help(
            f,
            &[
                "Drag / edge: pan   Wheel: zoom   1-5: tabs   P: pixel art",
                "Cliffs tile at true pixel size; water and lava animate in place in the atlas",
                "Team colour is painted magenta in the sheets and recoloured on load",
            ],
        );
        let size = f.ui_size();
        let h = if self.tab == Tab::Terrain { 84.0 } else { 212.0 };
        let r = Rect::new(size.x - 258.0, 106.0, 250.0, h);
        f.panel(r);
        let t = f.theme.clone();
        let mut y = r.y + 10.0;
        f.text(Vec2::new(r.x + 10.0, y), "Show", 8.0, t.text);
        y += 14.0;
        let tabs = Rect::new(r.x + 8.0, y, r.w - 16.0, 24.0).cols(3, 4.0);
        let tabs2 = Rect::new(r.x + 8.0, y + 28.0, r.w - 16.0, 24.0).cols(3, 4.0);
        for (i, (tab, name)) in TABS.iter().enumerate() {
            let cell = if i < 3 { tabs[i] } else { tabs2[i - 3] };
            f.button_ex(name, cell, name, true, self.tab == *tab);
        }
        y += 60.0;
        if self.tab == Tab::Terrain {
            return;
        }
        f.text(Vec2::new(r.x + 10.0, y), "Team colour", 8.0, t.text);
        y += 14.0;
        let c = Rect::new(r.x + 8.0, y, r.w - 16.0, 24.0).cols(3, 4.0);
        for (i, (name, _)) in TEAMS.iter().enumerate() {
            f.button_ex(name, c[i], name, true, self.team == i);
        }
        y += 34.0;
        if self.tab == Tab::Buildings {
            f.text(Vec2::new(r.x + 10.0, y), "State", 8.0, t.text);
            y += 14.0;
            let c = Rect::new(r.x + 8.0, y, r.w - 16.0, 24.0).cols(2, 4.0);
            let c2 = Rect::new(r.x + 8.0, y + 28.0, r.w - 16.0, 24.0).cols(2, 4.0);
            for (i, s) in STATES.iter().enumerate() {
                f.button_ex(s, if i < 2 { c[i] } else { c2[i - 2] }, s, true, self.state == i);
            }
            return;
        }
        f.text(Vec2::new(r.x + 10.0, y), "Animation", 8.0, t.text);
        y += 14.0;
        let names = ["Idle", "Walk", "Attack", "Hurt", "Death", "Special"];
        let c = Rect::new(r.x + 8.0, y, r.w - 16.0, 24.0).cols(3, 4.0);
        let c2 = Rect::new(r.x + 8.0, y + 28.0, r.w - 16.0, 24.0).cols(3, 4.0);
        for (i, n) in names.iter().enumerate() {
            f.button_ex(&format!("anim{i}"), if i < 3 { c[i] } else { c2[i - 3] }, n, true, self.anim == i);
        }
    }
}

const DIGITS: [Key; 5] = [Key::Digit1, Key::Digit2, Key::Digit3, Key::Digit4, Key::Digit5];
