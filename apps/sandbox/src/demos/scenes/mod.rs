//! Scenes & UI levels: an adventure map, a town screen and a hex battle, each its own scene on
//! the scene stack (the kind of layering strategy games like Heroes use). Results travel back down the stack with `Transition::PopWith`.

pub mod battle;
pub mod map;
pub mod town;

use crate::art::UnitKind;

/// A stack of identical creatures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stack {
    pub kind: UnitKind,
    pub count: u32,
}

impl Stack {
    pub const fn new(kind: UnitKind, count: u32) -> Self {
        Stack { kind, count }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub hp: u32,
    pub min_dmg: u32,
    pub max_dmg: u32,
    pub speed: i32,
    pub ranged: bool,
    pub flying: bool,
    pub gold: u32,
}

pub fn stats(kind: UnitKind) -> Stats {
    let s =
        |hp, min_dmg, max_dmg, speed, ranged, flying, gold| Stats { hp, min_dmg, max_dmg, speed, ranged, flying, gold };
    match kind {
        UnitKind::Footman => s(10, 1, 3, 4, false, false, 60),
        UnitKind::Archer => s(10, 2, 3, 4, true, false, 100),
        UnitKind::Ogre => s(25, 3, 5, 4, false, false, 200),
        UnitKind::Knight => s(30, 6, 9, 7, false, false, 400),
        UnitKind::Griffon => s(60, 12, 18, 9, false, true, 1200),
        UnitKind::Wolf => s(8, 2, 4, 6, false, false, 50),
        UnitKind::Paladin => s(40, 5, 8, 5, false, false, 0),
    }
}

/// Gold, wood, ore.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Purse {
    pub gold: u32,
    pub wood: u32,
    pub ore: u32,
}

impl Purse {
    pub const fn new(gold: u32, wood: u32, ore: u32) -> Self {
        Purse { gold, wood, ore }
    }

    pub fn covers(&self, cost: Purse) -> bool {
        self.gold >= cost.gold && self.wood >= cost.wood && self.ore >= cost.ore
    }

    pub fn pay(&mut self, cost: Purse) {
        self.gold -= cost.gold;
        self.wood -= cost.wood;
        self.ore -= cost.ore;
    }
}

/// What the town screen hands back to the map.
pub struct TownResult {
    pub purse: Purse,
    pub army: Vec<Stack>,
    pub built: [bool; 5],
}

/// What a battle hands back to the map.
pub struct BattleResult {
    pub won: bool,
    pub army: Vec<Stack>,
}

/// Merge a stack into an army (max 7 slots).
pub fn add_to_army(army: &mut Vec<Stack>, s: Stack) -> bool {
    if let Some(x) = army.iter_mut().find(|x| x.kind == s.kind) {
        x.count += s.count;
        true
    } else if army.len() < 7 {
        army.push(s);
        true
    } else {
        false
    }
}
