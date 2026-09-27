//! The sandbox demos. Each is a [`knight_engine::Scene`] pushed from the menu and named after the
//! engine features it shows.

use std::rc::Rc;

use knight_engine::{Key, Scene};

use crate::art::Art;

pub mod asset_pack;
pub mod battle_royale;
pub mod chat;
pub mod editor;
pub mod hybrid;
pub mod menu;
pub mod online;
pub mod overlays;
pub mod pathfinding;
pub mod realtime_multi;
pub mod realtime_single;
pub mod routes;
pub mod scenes;
pub mod stress;
pub mod terrain;
pub mod turn_multi;
pub mod turn_single;
pub mod vision;

/// A menu entry: shortcut key, label, title and blurb.
pub struct Entry {
    pub key: Key,
    pub label: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
    pub open: fn(Rc<Art>) -> Box<dyn Scene>,
}

pub const ENTRIES: [Entry; 17] = [
    Entry {
        key: Key::Digit1,
        label: "1",
        title: "Terrain & Camera",
        blurb: "Heights, water, map shapes, pointy/flat hexes, view angle, pixel art vs hi-res",
        open: |a| Box::new(terrain::TerrainDemo::new(a)),
    },
    Entry {
        key: Key::Digit2,
        label: "2",
        title: "Picking & Editing",
        blurb: "Elevation-aware picking; sculpt heights, paint, add and erase tiles",
        open: |a| Box::new(editor::EditorDemo::new(a)),
    },
    Entry {
        key: Key::Digit3,
        label: "3",
        title: "Pathfinding",
        blurb: "A* with terrain costs and climb limits, move ranges, flow fields",
        open: |a| Box::new(pathfinding::PathDemo::new(a)),
    },
    Entry {
        key: Key::Digit4,
        label: "4",
        title: "Vision & Fog",
        blurb: "Height-aware line of sight, field of view, fog of war with memory",
        open: |a| Box::new(vision::VisionDemo::new(a)),
    },
    Entry {
        key: Key::Digit5,
        label: "5",
        title: "Overlays, Light & FX",
        blurb: "Territory borders, hex-edge rivers, day/night light, particles",
        open: |a| Box::new(overlays::OverlaysDemo::new(a)),
    },
    Entry {
        key: Key::Digit6,
        label: "6",
        title: "Scenes & UI Levels",
        blurb: "Map -> town -> battle as a scene stack with results passed back",
        open: |a| Box::new(scenes::map::MapScene::new(a)),
    },
    Entry {
        key: Key::Digit7,
        label: "7",
        title: "Turn-Based: 1 Unit",
        blurb: "Move and strike, then monsters act; threat ranges; terrain costs",
        open: |a| Box::new(turn_single::TurnSingle::new(a)),
    },
    Entry {
        key: Key::Digit8,
        label: "8",
        title: "Turn-Based: Squads",
        blurb: "Box select, group orders, each unit acts once; enemy turns, fog",
        open: |a| Box::new(turn_multi::TurnMulti::new(a)),
    },
    Entry {
        key: Key::Digit9,
        label: "9",
        title: "Realtime: 1 Unit",
        blurb: "Real-time hex movement at terrain speed, chase and strike, pause",
        open: |a| Box::new(realtime_single::RealtimeSingle::new(a)),
    },
    Entry {
        key: Key::Digit0,
        label: "0",
        title: "Realtime: Squads",
        blurb: "Hex reservations (no overlap), group spread, minimap, fog, pause",
        open: |a| Box::new(realtime_multi::RealtimeMulti::new(a)),
    },
    Entry {
        key: Key::H,
        label: "H",
        title: "Hybrid: Timed Days",
        blurb: "Everyone acts at once with a daily budget; the timer is the turn",
        open: |a| Box::new(hybrid::HybridDemo::new(a)),
    },
    Entry {
        key: Key::B,
        label: "B",
        title: "Battle Royale",
        blurb: "Free-for-all; a ring of fire shrinks toward a random point",
        open: |a| Box::new(battle_royale::BattleRoyale::new(a)),
    },
    Entry {
        key: Key::C,
        label: "C",
        title: "Chat & Emotes",
        blurb: "Multiplayer chat bubbles and emotes that stay readable in crowds",
        open: |a| Box::new(chat::ChatDemo::new(a)),
    },
    Entry {
        key: Key::O,
        label: "O",
        title: "Online Play",
        blurb: "Relay rooms over WebSocket: shared moves, chat and emotes, ping",
        open: |a| Box::new(online::OnlineDemo::new(a)),
    },
    Entry {
        key: Key::R,
        label: "R",
        title: "Routes & Transport",
        blurb: "Survey routes over build costs, lay track, run vehicles along them",
        open: |a| Box::new(routes::RoutesDemo::new(a)),
    },
    Entry {
        key: Key::S,
        label: "S",
        title: "Stress Test",
        blurb: "Up to 1M hexes (only visible chunks meshed), minimap, 2000 units pathing hex to hex",
        open: |a| Box::new(stress::StressDemo::new(a)),
    },
    Entry {
        key: Key::A,
        label: "A",
        title: "Asset Pack",
        blurb: "Bundled pixel art: 19 terrains, props, fire, 45 characters, 38 buildings, team colours",
        open: |a| Box::new(asset_pack::AssetPackDemo::new(a)),
    },
];
