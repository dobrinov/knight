//! Turn based, multi unit: box-select squads, give group move / attack
//! orders (each unit moves and attacks once per turn), then the enemy takes its turn. Fog of war
//! comes from unit vision; the minimap is a second camera rendering the same world.

use std::collections::HashMap;
use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path::Reachable;
use knight_engine::hex::{Hex, HexSet, Rng};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{self, Decor, MapGen};
use crate::tactics::{self, Action, Director, Unit};

const AGGRO: i32 = 7;
const VISION: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Player,
    Enemy,
}

pub struct TurnMulti {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    camera: Camera,
    minimap: Camera,
    controller: CameraController,
    units: Vec<Unit>,
    director: Director,
    phase: Phase,
    turn: u32,
    selected: Vec<usize>,
    groups: HashMap<usize, Vec<usize>>,
    box_select: Option<(Vec2, Vec2)>,
    hover: Option<Pick>,
    /// Movement ranges of selected units that can still move.
    reach: Vec<(usize, Reachable)>,
    preview: Vec<Action>,
    base: Hex,
    enemy_base: Hex,
}

impl TurnMulti {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen {
                seed: 77,
                size: 26,
                amplitude: 6.0,
                island: 0.55,
                trees: 0.5,
                shape: common::MapShape::Rectangle,
                ..Default::default()
            },
        );
        world.set_fog_enabled(true);
        let mut land: Vec<Hex> =
            world.hexes().filter(|&h| world.tile(h).is_some_and(|t| (1..=3).contains(&t.height))).collect();
        land.sort();
        let (lo, hi) = world.bounds();
        let near = |p: Vec2| {
            *land
                .iter()
                .min_by_key(|h| {
                    let q = world.layout.hex_to_point(**h);
                    (((q.x - p.x).powi(2) + (q.y - p.y).powi(2)) * 100.0) as i64
                })
                .unwrap()
        };
        let base = near(Vec2::new(lo.x + (hi.x - lo.x) * 0.22, lo.y + (hi.y - lo.y) * 0.6));
        let enemy_base = near(Vec2::new(lo.x + (hi.x - lo.x) * 0.8, lo.y + (hi.y - lo.y) * 0.35));
        let mut decor = decor;
        decor.retain(|d| d.hex.distance(base) > 3 && d.hex.distance(enemy_base) > 2);

        let mut rng = Rng::new(9);
        let mut units: Vec<Unit> = Vec::new();
        let place = |units: &mut Vec<Unit>, center: Hex, kinds: &[(UnitKind, usize)], team: Team, rng: &mut Rng| {
            let mut spots: Vec<Hex> = center
                .spiral(4)
                .into_iter()
                .filter(|&h| h != center && world.tile(h).is_some_and(|t| t.height >= 1) && !world.is_submerged(h))
                .filter(|&h| units.iter().all(|u| u.hex != h))
                .collect();
            rng.shuffle(&mut spots);
            let mut spots = spots.into_iter();
            for &(kind, n) in kinds {
                for _ in 0..n {
                    if let Some(h) = spots.next() {
                        let mut u = Unit::new(&world, kind, team, h);
                        u.actor.scale = 0.85;
                        units.push(u);
                    }
                }
            }
        };
        place(
            &mut units,
            base,
            &[(UnitKind::Footman, 6), (UnitKind::Archer, 5), (UnitKind::Knight, 3)],
            Team::Blue,
            &mut rng,
        );
        for pack in 0..3 {
            let center = land[(land.len() / 4) * (pack + 1)];
            place(&mut units, center, &[(UnitKind::Wolf, 4)], Team::Red, &mut rng);
        }
        place(&mut units, enemy_base, &[(UnitKind::Ogre, 4), (UnitKind::Archer, 2)], Team::Red, &mut rng);

        let camera = Camera::new(world.hex_to_world(base), 40.0).with_pitch(52.0);
        let mut minimap = Camera::new(world.center(), 4.0).with_pitch(90.0);
        minimap.min_zoom = 0.5;
        let controller = CameraController::default().drag_with(&[MouseButton::Middle]);
        let mut demo = TurnMulti {
            art,
            world,
            decor,
            camera,
            minimap,
            controller,
            units,
            director: Director::new(11),
            phase: Phase::Player,
            turn: 1,
            selected: Vec::new(),
            groups: HashMap::new(),
            box_select: None,
            hover: None,
            reach: Vec::new(),
            preview: Vec::new(),
            base,
            enemy_base,
        };
        demo.update_fog();
        demo
    }

    fn update_fog(&mut self) {
        let mut visible = HexSet::default();
        for u in self.units.iter().filter(|u| u.team() == Team::Blue && u.alive()) {
            visible.extend(u.hex.spiral(VISION));
        }
        visible.extend(self.base.spiral(5));
        let visible: Vec<Hex> = visible.into_iter().filter(|h| self.world.contains(*h)).collect();
        self.world.update_fog(visible);
    }

    fn minimap_rect(ui: Vec2) -> Rect {
        Rect::new(ui.x - 228.0, ui.y - 228.0, 220.0, 220.0)
    }

    fn refresh_reach(&mut self) {
        self.reach = self
            .selected
            .iter()
            .filter(|&&i| !self.units[i].moved)
            .map(|&i| (i, tactics::reach(&self.world, &self.units, i, self.units[i].moves)))
            .collect();
    }

    /// Orders for the selection when clicking `pick`: attack an enemy with everyone who can
    /// reach it, or spread the group around a destination hex.
    fn orders(&self, pick: Pick) -> Vec<Action> {
        let occ = tactics::occupancy(&self.units);
        let mut out = Vec::new();
        if let Some(&t) = occ.get(pick.hex)
            && self.units[t].team() == Team::Red
        {
            // Each attacker plans against the board after earlier attackers moved.
            let mut units = self.units.clone();
            for &i in &self.selected {
                let r = if units[i].moved {
                    Reachable::default()
                } else {
                    tactics::reach(&self.world, &units, i, units[i].moves)
                };
                let plan = tactics::player_command(&self.world, &units, i, &r, pick);
                for a in &plan {
                    if let Action::Move { unit, path } = a {
                        units[*unit].hex = *path.last().unwrap();
                    }
                }
                out.extend(plan);
            }
            return out;
        }
        if !self.world.contains(pick.hex) {
            return out;
        }
        // Group move: closest units claim the free reachable hexes nearest the goal.
        let mut movers: Vec<usize> = self.selected.iter().copied().filter(|&i| !self.units[i].moved).collect();
        movers.sort_by_key(|&i| self.units[i].hex.distance(pick.hex));
        let mut units = self.units.clone();
        for i in movers {
            let r = tactics::reach(&self.world, &units, i, units[i].moves);
            let best = r.hexes().min_by_key(|&h| (h.distance(pick.hex), r.cost(h).unwrap_or(0), h.q, h.r));
            if let Some(h) = best
                && h != units[i].hex
                && h.distance(pick.hex) < units[i].hex.distance(pick.hex)
            {
                let path = r.path_to(h).unwrap_or_default();
                units[i].hex = h;
                out.push(Action::Move { unit: i, path });
            }
        }
        out
    }

    fn issue(&mut self, orders: Vec<Action>) {
        for a in orders {
            match &a {
                Action::Move { unit, .. } => self.units[*unit].moved = true,
                Action::Attack { unit, .. } => {
                    self.units[*unit].acted = true;
                    self.units[*unit].moved = true;
                }
                Action::Nova { .. } => {}
            }
            self.director.push(a);
        }
        self.reach.clear();
    }

    fn end_turn(&mut self) {
        self.phase = Phase::Enemy;
        self.preview.clear();
        self.reach.clear();
        let players: Vec<Hex> =
            self.units.iter().filter(|u| u.team() == Team::Blue && u.alive()).map(|u| u.hex).collect();
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.team() == Team::Red && u.alive() && players.iter().any(|p| p.distance(u.hex) <= AGGRO) {
                for a in tactics::ai_turn(&self.world, &self.units, i) {
                    if let Action::Move { unit, path } = &a {
                        self.units[*unit].hex = *path.last().unwrap();
                    }
                    self.director.push(a);
                }
            }
        }
        for u in &mut self.units {
            u.hex = u.actor.hex();
        }
    }
}

impl Scene for TurnMulti {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let dt = ctx.time.dt;
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let ui = ctx.screen / ctx.ui_scale();
        let mm = Self::minimap_rect(ui).scale(ctx.ui_scale());
        self.minimap.viewport = mm;
        let (lo, hi) = self.world.bounds();
        self.minimap.target = Vec3::new((lo.x + hi.x) * 0.5, 0.0, (lo.y + hi.y) * 0.5);
        self.minimap.zoom = (mm.w / (hi.x - lo.x + 2.0)).min(mm.h / (hi.y - lo.y + 2.0));
        self.minimap.goal_zoom = self.minimap.zoom;

        let input = &ctx.input;
        let over_ui = input.pointer_over_ui();
        let on_minimap = mm.contains(input.mouse);
        if on_minimap
            && input.down(MouseButton::Left)
            && let Some(g) = self.minimap.ground_at(input.mouse, 0.0)
        {
            self.camera.target = Vec3::new(g.x, self.camera.target.y, g.z);
        }
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        self.hover = if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)) };

        let moving_before = self.director.busy();
        for e in self.director.update(&self.world, &self.art, &mut self.units, dt) {
            if let tactics::Event::Struck { pos, ranged } = e {
                common::strike_sound(ctx, &self.art, pos, ranged, &self.camera);
            }
        }
        if moving_before && !self.director.busy() {
            self.update_fog();
        }
        self.selected.retain(|&i| self.units[i].alive());

        let input = &ctx.input;
        if self.phase == Phase::Player && !self.director.busy() {
            if self.reach.is_empty() && self.selected.iter().any(|&i| !self.units[i].moved) {
                self.refresh_reach();
            }
            let own_under_cursor = self
                .hover
                .and_then(|p| tactics::occupancy(&self.units).get(p.hex).copied())
                .filter(|&i| self.units[i].team() == Team::Blue);
            self.preview = match self.hover {
                Some(p) if !self.selected.is_empty() && own_under_cursor.is_none() => self.orders(p),
                _ => Vec::new(),
            };
            if !over_ui && !on_minimap {
                if let Some((a, b)) = input.drag(MouseButton::Left) {
                    self.box_select = Some((a, b));
                }
                if let Some((a, b)) = input.drag_released(MouseButton::Left) {
                    let r = Rect::from_corners(a, b);
                    if !input.shift() {
                        self.selected.clear();
                    }
                    for (i, u) in self.units.iter().enumerate() {
                        let s = self.camera.world_to_screen(u.actor.pos + Vec3::Y * 0.5);
                        if u.team() == Team::Blue && u.alive() && r.contains(s) && !self.selected.contains(&i) {
                            self.selected.push(i);
                        }
                    }
                    self.box_select = None;
                    self.reach.clear();
                } else if input.clicked(MouseButton::Left) || input.clicked(MouseButton::Right) {
                    let right = input.clicked(MouseButton::Right);
                    if let Some(i) = own_under_cursor.filter(|_| !right) {
                        if !input.shift() {
                            self.selected.clear();
                        }
                        if !self.selected.contains(&i) {
                            self.selected.push(i);
                        }
                        self.reach.clear();
                    } else if !self.preview.is_empty() {
                        let orders = std::mem::take(&mut self.preview);
                        self.issue(orders);
                    } else if !right {
                        self.selected.clear();
                        self.reach.clear();
                    }
                }
            }
            let input = &ctx.input;
            if let Some(d) = input.digit_pressed() {
                if input.ctrl() {
                    self.groups.insert(d, self.selected.clone());
                } else if let Some(g) = self.groups.get(&d) {
                    self.selected = g.clone();
                    self.reach.clear();
                }
            }
            if input.key_pressed(Key::Tab) {
                self.selected = (0..self.units.len())
                    .filter(|&i| self.units[i].team() == Team::Blue && self.units[i].alive())
                    .collect();
                self.reach.clear();
            }
            if input.key_pressed(Key::Enter) || input.key_pressed(Key::Space) || input.ui_clicked("endturn") {
                self.end_turn();
            }
        } else if self.phase == Phase::Enemy && !self.director.busy() {
            self.turn += 1;
            self.phase = Phase::Player;
            for u in &mut self.units {
                u.moved = false;
                u.acted = false;
            }
            self.reach.clear();
            self.update_fog();
        }
        if !ctx.input.down(MouseButton::Left) {
            self.box_select = None;
        }
        if ctx.input.key_pressed(Key::H) {
            self.camera.move_to(self.world.hex_to_world(self.base));
        }
        let alive = |t: Team| self.units.iter().filter(|u| u.team() == t && u.alive()).count();
        ctx.status = format!(
            "strategy turn={} phase={:?} blue={} red={} selected={}",
            self.turn,
            self.phase,
            alive(Team::Blue),
            alive(Team::Red),
            self.selected.len()
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let visible: Vec<bool> =
            self.units.iter().map(|u| u.team() == Team::Blue || self.world.is_visible(u.actor.hex())).collect();
        let (bx, ex) = (self.base, self.enemy_base);
        let player_turn = self.phase == Phase::Player && !self.director.busy();
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
            w.sprite(Sprite::new(self.art.keep[0], w.world.hex_to_world(bx)).scale(1.4));
            if w.world.visibility(ex) != Visibility::Hidden {
                w.sprite(Sprite::new(self.art.keep[1], w.world.hex_to_world(ex)).scale(1.4));
            }
            if player_turn {
                for (_, r) in &self.reach {
                    for h in r.hexes() {
                        w.hex_fill_inset(h, Color::hex(0x9fd8ff).with_alpha(0.16), 0.08);
                    }
                }
                w.set_on_top(true);
                for a in &self.preview {
                    match a {
                        Action::Move { path, .. } => {
                            w.path(path, 0.07, Color::WHITE.with_alpha(0.75), false);
                            w.hex_outline(*path.last().unwrap(), Color::WHITE.with_alpha(0.9), 0.08);
                        }
                        Action::Attack { target, .. } => w.hex_outline(self.units[*target].hex, Color::RED, 0.14),
                        Action::Nova { .. } => {}
                    }
                }
                w.set_on_top(false);
            }
            for (i, u) in self.units.iter_mut().enumerate() {
                if !visible[i] {
                    continue;
                }
                u.actor.selected = self.selected.contains(&i);
                // Units that are done for this turn are greyed out.
                u.actor.tint =
                    if u.team() == Team::Blue && u.done() { Color::rgb(0.55, 0.55, 0.6) } else { Color::WHITE };
                u.actor.draw(&mut w, &self.art, true);
            }
            self.director.draw(&mut w, &self.art, &self.units);
        }
        let cam = self.camera.clone();
        {
            let mut m = f.world(&mut self.world, &self.minimap);
            m.background(Color::hex(0x0a0a10));
            m.set_on_top(true);
            for (i, u) in self.units.iter().enumerate() {
                if u.alive() && visible[i] {
                    let col = if u.team() == Team::Blue { Color::hex(0x6fb0ff) } else { Color::hex(0xff5a4a) };
                    m.ring(u.actor.pos, 0.9, col);
                }
            }
            let v = cam.viewport;
            let corners = [
                Vec2::new(v.x, v.y),
                Vec2::new(v.x + v.w, v.y),
                Vec2::new(v.x + v.w, v.y + v.h),
                Vec2::new(v.x, v.y + v.h),
            ];
            let g: Vec<Option<Vec3>> = corners.iter().map(|&c| cam.ground_at(c, 0.0)).collect();
            for k in 0..4 {
                if let (Some(a), Some(b)) = (g[k], g[(k + 1) % 4]) {
                    m.line(a, b, 0.5, Color::WHITE.with_alpha(0.8));
                }
            }
        }
        let size = f.ui_size();
        let mm = Self::minimap_rect(size);
        f.block(mm);
        f.outline(mm.inset(-2.0), f.theme.panel_border, 2.0);
        if let Some((a, b)) = self.box_select {
            let s = f.ui_scale();
            let r = Rect::from_corners(a / s, b / s);
            f.rect(r, Color::hex(0x7cff7c).with_alpha(0.12));
            f.outline(r, Color::hex(0x7cff7c), 1.0);
        }
        common::header(f, "Turn-Based - Multi Unit", "Each unit moves and attacks once per turn, then the enemy moves");
        common::stats(f);
        common::help(
            f,
            &[
                "Left drag: box select   Click unit: select   Shift: add   Tab: all",
                "Click / right click a hex: group move   ...an enemy: attack",
                "Enter: end turn   Ctrl+1-9 / 1-9: groups   Middle drag / WASD / edges: scroll",
            ],
        );
        let t = f.theme.clone();
        let blue = self.units.iter().filter(|u| u.team() == Team::Blue && u.alive()).count();
        let red = self.units.iter().filter(|u| u.team() == Team::Red && u.alive()).count();
        let ready = self.units.iter().filter(|u| u.team() == Team::Blue && u.alive() && !u.done()).count();
        let r = Rect::new(size.x - 208.0, 106.0, 200.0, 92.0);
        f.panel(r);
        let title = if red == 0 {
            "Victory!".to_string()
        } else if blue == 0 {
            "Defeat".to_string()
        } else if player_turn {
            format!("Turn {} - your move", self.turn)
        } else {
            "Enemy turn...".to_string()
        };
        f.text(Vec2::new(r.x + 10.0, r.y + 10.0), &title, 8.0, t.accent);
        f.text(Vec2::new(r.x + 10.0, r.y + 26.0), &format!("{ready}/{blue} ready, {red} foes"), 8.0, t.text_dim);
        f.button_ex(
            "endturn",
            Rect::new(r.x + 8.0, r.y + 46.0, r.w - 16.0, 36.0),
            "End turn [Enter]",
            player_turn,
            false,
        );

        if !self.selected.is_empty() {
            let mut counts: Vec<(UnitKind, usize, f32)> = Vec::new();
            for &i in &self.selected {
                let a = &self.units[i].actor;
                match counts.iter_mut().find(|c| c.0 == a.kind) {
                    Some(c) => {
                        c.1 += 1;
                        c.2 += a.hp / a.max_hp;
                    }
                    None => counts.push((a.kind, 1, a.hp / a.max_hp)),
                }
            }
            let r = Rect::new(size.x * 0.5 - 150.0, size.y - 92.0, 300.0, 84.0);
            f.panel(r);
            f.text(Vec2::new(r.x + 10.0, r.y + 8.0), &format!("{} selected", self.selected.len()), 8.0, t.accent);
            for (k, (kind, n, hp)) in counts.iter().enumerate() {
                let c = Rect::new(r.x + 10.0 + k as f32 * 70.0, r.y + 20.0, 64.0, 58.0);
                f.rect(c, Color::hex(0x2a2432));
                f.image_fit(
                    Rect::new(c.x, c.y - 2.0, c.w, c.h - 10.0),
                    self.art.unit(*kind, Team::Blue).idle.frames[0],
                    Color::WHITE,
                );
                f.text(Vec2::new(c.x + 4.0, c.y + c.h - 18.0), &format!("x{n}"), 8.0, Color::WHITE);
                f.bar(
                    Rect::new(c.x + 4.0, c.y + c.h - 7.0, c.w - 8.0, 4.0),
                    hp / *n as f32,
                    Color::hex(0x6ee06e),
                    Color::hex(0x202020),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ending turns repeatedly: the enemy AI acts and the turn loop keeps going.
    #[test]
    fn enemy_turns_run() {
        let mut ctx = Context::new();
        let art = Art::load(&mut ctx);
        let mut d = TurnMulti::new(art);
        ctx.time.dt = 1.0 / 20.0;
        // Send the whole army towards the enemy base for a few turns.
        for _ in 0..4000 {
            if d.phase == Phase::Player && !d.director.busy() {
                d.selected =
                    (0..d.units.len()).filter(|&i| d.units[i].team() == Team::Blue && d.units[i].alive()).collect();
                let goal = d.world.hex_to_world(d.enemy_base);
                let orders = d.orders(Pick { hex: d.enemy_base, point: goal });
                d.issue(orders);
                ctx.input.on_key(Key::Enter, true);
            }
            let _ = d.update(&mut ctx);
            ctx.input.end_frame();
            ctx.input.on_key(Key::Enter, false);
            if d.turn > 10 {
                break;
            }
        }
        assert!(d.turn > 5, "turns advanced: {}", d.turn);
        // The defenders noticed the army and fought it.
        let hurt = d.units.iter().any(|u| u.actor.hp < u.actor.max_hp);
        assert!(hurt, "combat happened");
    }
}
