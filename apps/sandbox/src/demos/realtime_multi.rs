//! Real time, multi unit: command squads in real time on the hex grid. Box-select, then order a
//! group to a hex (it spreads over distinct hexes) or onto an enemy. Units reserve hexes as
//! they walk, so crowds queue and flow around each other; terrain sets their speed. Fog of war
//! from unit vision, a clickable minimap and real-time-with-pause.

use std::collections::HashMap;
use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::{Hex, Rng};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{self, Decor, MapGen};
use crate::realtime::{self, Order, RtWorld};

pub struct RealtimeMulti {
    art: Rc<Art>,
    world: HexWorld,
    decor: Vec<Decor>,
    camera: Camera,
    minimap: Camera,
    controller: CameraController,
    rt: RtWorld,
    selected: Vec<usize>,
    groups: HashMap<usize, Vec<usize>>,
    box_select: Option<(Vec2, Vec2)>,
    marker: Option<(Vec3, f32, bool)>,
    fog_timer: f32,
    base: Hex,
    enemy_base: Hex,
    resume_speed: f32,
}

impl RealtimeMulti {
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

        let mut rt = RtWorld::new();
        let mut rng = Rng::new(9);
        let mut place = |rt: &mut RtWorld, center: Hex, kinds: &[(UnitKind, usize)], team: Team| {
            let mut spots: Vec<Hex> = center
                .spiral(4)
                .into_iter()
                .filter(|&h| h != center && world.tile(h).is_some_and(|t| t.height >= 1))
                .collect();
            rng.shuffle(&mut spots);
            let mut spots = spots.into_iter();
            for &(kind, n) in kinds {
                let mut placed = 0;
                while placed < n {
                    let Some(h) = spots.next() else { break };
                    if rt.spawn(&world, kind, team, h).is_some() {
                        placed += 1;
                    }
                }
            }
        };
        place(&mut rt, base, &[(UnitKind::Footman, 10), (UnitKind::Archer, 8), (UnitKind::Knight, 4)], Team::Blue);
        for pack in 0..3 {
            place(&mut rt, land[(land.len() / 4) * (pack + 1)], &[(UnitKind::Wolf, 6)], Team::Red);
        }
        place(&mut rt, enemy_base, &[(UnitKind::Ogre, 5), (UnitKind::Archer, 3)], Team::Red);

        let camera = Camera::new(world.hex_to_world(base), 40.0).with_pitch(52.0);
        let mut minimap = Camera::new(world.center(), 4.0).with_pitch(90.0);
        minimap.min_zoom = 0.5;
        let controller = CameraController::rts();
        let mut demo = RealtimeMulti {
            art,
            world,
            decor,
            camera,
            minimap,
            controller,
            rt,
            selected: Vec::new(),
            groups: HashMap::new(),
            box_select: None,
            marker: None,
            fog_timer: 0.0,
            base,
            enemy_base,
            resume_speed: 1.0,
        };
        demo.update_fog();
        demo
    }

    fn update_fog(&mut self) {
        let mut visible = self.rt.vision(Team::Blue, 5);
        visible.extend(self.base.spiral(5));
        let visible: Vec<Hex> = visible.into_iter().filter(|h| self.world.contains(*h)).collect();
        self.world.update_fog(visible);
    }

    fn minimap_rect(ui: Vec2) -> Rect {
        Rect::new(ui.x - 228.0, ui.y - 228.0, 220.0, 220.0)
    }
}

impl Scene for RealtimeMulti {
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
        // Control groups use Ctrl+digits; plain digits recall groups, so speed is on the buttons.
        let input = &ctx.input;
        if input.key_pressed(Key::Space) || input.ui_clicked("pause") {
            if self.rt.speed > 0.0 {
                self.resume_speed = self.rt.speed;
                self.rt.speed = 0.0;
            } else {
                self.rt.speed = self.resume_speed.max(1.0);
            }
        }
        for (id, v) in [("speed1", 1.0), ("speed2", 2.0), ("speed4", 4.0)] {
            if input.ui_clicked(id) {
                self.rt.speed = v;
            }
        }

        if !over_ui && !on_minimap {
            if let Some((a, b)) = input.drag(MouseButton::Left) {
                self.box_select = Some((a, b));
            }
            if let Some((a, b)) = input.drag_released(MouseButton::Left) {
                let r = Rect::from_corners(a, b);
                if !input.shift() {
                    self.selected.clear();
                }
                for (i, u) in self.rt.units.iter().enumerate() {
                    let s = self.camera.world_to_screen(u.actor.pos + Vec3::Y * 0.5);
                    if u.team() == Team::Blue && u.alive() && r.contains(s) && !self.selected.contains(&i) {
                        self.selected.push(i);
                    }
                }
                self.box_select = None;
            } else if input.clicked(MouseButton::Left) {
                let hit = self
                    .world
                    .pick(self.camera.screen_ray(input.mouse))
                    .and_then(|p| self.rt.unit_at(p.hex))
                    .filter(|&i| self.rt.units[i].team() == Team::Blue);
                if !input.shift() {
                    self.selected.clear();
                }
                if let Some(i) = hit {
                    self.selected.push(i);
                }
            }
            if input.clicked(MouseButton::Right)
                && !self.selected.is_empty()
                && let Some(p) = self.world.pick(self.camera.screen_ray(input.mouse))
            {
                match self.rt.unit_at(p.hex).filter(|&i| self.rt.units[i].team() == Team::Red) {
                    Some(e) => {
                        self.rt.order_attack(&self.selected, e);
                        self.marker = Some((self.rt.units[e].actor.pos, 0.0, true));
                    }
                    None => {
                        self.rt.order_move(&self.world, &self.selected, p.hex);
                        self.marker = Some((p.point, 0.0, false));
                    }
                }
            }
        }
        if !input.down(MouseButton::Left) {
            self.box_select = None;
        }
        if let Some(d) = input.digit_pressed() {
            if input.ctrl() {
                self.groups.insert(d, self.selected.clone());
            } else if let Some(g) = self.groups.get(&d) {
                self.selected = g.clone();
            }
        }
        if input.key_pressed(Key::Tab) {
            self.selected = (0..self.rt.units.len())
                .filter(|&i| self.rt.units[i].team() == Team::Blue && self.rt.units[i].alive())
                .collect();
        }
        if input.key_pressed(Key::S) && input.shift() {
            for &i in &self.selected {
                self.rt.units[i].order = Order::Idle;
                self.rt.units[i].actor.mover.stop();
            }
        }
        if input.key_pressed(Key::H) {
            self.camera.move_to(self.world.hex_to_world(self.base));
        }

        for e in self.rt.step(&self.world, &self.art, dt) {
            if let crate::realtime::RtEvent::Struck { pos, ranged } = e {
                common::strike_sound(ctx, &self.art, pos, ranged, &self.camera);
            }
        }
        self.selected.retain(|&i| self.rt.units[i].alive());
        if let Some((_, t, _)) = &mut self.marker {
            *t += dt;
            if *t > 0.6 {
                self.marker = None;
            }
        }
        self.fog_timer -= dt;
        if self.fog_timer <= 0.0 {
            self.fog_timer = 0.15;
            self.update_fog();
        }
        let alive = |t: Team| self.rt.units.iter().filter(|u| u.team() == t && u.alive()).count();
        ctx.status = format!(
            "realtime-multi blue={} red={} selected={} moving={} speed={}",
            alive(Team::Blue),
            alive(Team::Red),
            self.selected.len(),
            self.rt.units.iter().filter(|u| u.actor.moving()).count(),
            self.rt.speed
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let visible: Vec<bool> =
            self.rt.units.iter().map(|u| u.team() == Team::Blue || self.world.is_visible(u.hex())).collect();
        let (bx, ex) = (self.base, self.enemy_base);
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
            w.sprite(Sprite::new(self.art.keep[0], w.world.hex_to_world(bx)).scale(1.4));
            if w.world.visibility(ex) != Visibility::Hidden {
                w.sprite(Sprite::new(self.art.keep[1], w.world.hex_to_world(ex)).scale(1.4));
            }
            // Routes of the selection.
            w.set_on_top(true);
            for &i in &self.selected {
                let m = &self.rt.units[i].actor.mover;
                let route: Vec<Hex> = std::iter::once(m.step().map_or(m.hex(), |s| s.0)).chain(m.route()).collect();
                if route.len() > 1 {
                    w.path(&route, 0.05, Color::hex(0x7cff7c).with_alpha(0.45), false);
                }
            }
            if let Some((p, t, attack)) = self.marker {
                let col = if attack { Color::RED } else { Color::hex(0x7cff7c) };
                w.ring(p, 0.6 - t * 0.5, col.with_alpha(1.0 - t / 0.6));
            }
            w.set_on_top(false);
            for (i, u) in self.rt.units.iter_mut().enumerate() {
                if !visible[i] {
                    continue;
                }
                u.actor.selected = self.selected.contains(&i);
                u.actor.draw(&mut w, &self.art, true);
            }
            self.rt.draw_fx(&mut w, &self.art);
        }
        let cam = self.camera.clone();
        {
            let mut m = f.world(&mut self.world, &self.minimap);
            m.background(Color::hex(0x0a0a10));
            m.set_on_top(true);
            for (i, u) in self.rt.units.iter().enumerate() {
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
        common::header(
            f,
            "Realtime - Multi Unit",
            "Squads in real time on hexes: reservations stop overlap, terrain sets speed",
        );
        common::stats(f);
        common::help(
            f,
            &[
                "Left drag: box select   Click: select   Shift: add   Tab: all   Shift+S: stop",
                "Right click: move (group spreads out) / attack   Ctrl+1-9: set group   1-9: recall",
                "Space: pause   Middle drag / WASD / screen edge: scroll   H: home   Minimap: jump",
            ],
        );
        realtime::draw_time_controls(f, &self.rt, Vec2::new(size.x - 208.0, 106.0));
        if !self.selected.is_empty() {
            let t = f.theme.clone();
            let mut counts: Vec<(UnitKind, usize, f32)> = Vec::new();
            for &i in &self.selected {
                let a = &self.rt.units[i].actor;
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
