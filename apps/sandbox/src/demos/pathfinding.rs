//! Pathfinding: A* with terrain costs and climb limits, budgeted movement ranges and flow fields
//! steering a pack of wolves.

use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path::{self, FlowField, Reachable};
use knight_engine::hex::{Hex, Rng};
use knight_engine::*;

use crate::art::{Art, Mats, Team, UnitKind};
use crate::common::{self, Actor, Decor, MapGen};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    AStar,
    Range,
    Flow,
}

pub struct PathDemo {
    art: Rc<Art>,
    world: HexWorld,
    mats: Mats,
    decor: Vec<Decor>,
    camera: Camera,
    controller: CameraController,
    mode: Mode,
    unit: Actor,
    hover: Option<Hex>,
    path: Option<path::Path>,
    range: Reachable,
    budget: u32,
    climb: i16,
    flow: Option<FlowField>,
    goal: Option<Hex>,
    wolves: Vec<Actor>,
    /// Wolves reserve hexes so the pack queues instead of overlapping.
    reservations: Reservations,
    t_search_us: f32,
}

/// Movement points for one step, or `None` if blocked: the world's terrain costs at 2 points
/// per normal step (road 1, grass 2, sand/rock 3, forest 4, snow 5), +50% per level climbed.
fn step_cost(world: &HexWorld, _mats: &Mats, climb: i16, from: Hex, to: Hex) -> Option<u32> {
    world.step_points(from, to, climb, 2)
}

impl PathDemo {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        let decor = common::generate(
            &mut world,
            &mats,
            &art,
            &MapGen { seed: 11, size: 16, amplitude: 9.0, island: 0.7, trees: 0.5, ..Default::default() },
        );
        // A road through the middle, to show cheap tiles.
        for q in -10..=10 {
            let h = Hex::new(q, -q / 2);
            if world.tile(h).is_some_and(|t| t.height > 0) {
                world.set_material(h, mats.road);
            }
        }
        let start = world.hexes().filter(|&h| !world.is_submerged(h)).min_by_key(|h| h.length()).unwrap_or(Hex::ORIGIN);
        let mut unit = Actor::new(UnitKind::Footman, Team::Blue, &world, start);
        unit.mover.speed = 2.5;
        let camera = Camera::new(world.hex_to_world(start), 40.0);
        PathDemo {
            art,
            world,
            mats,
            decor,
            camera,
            controller: CameraController::rts(),
            mode: Mode::AStar,
            unit,
            hover: None,
            path: None,
            range: Reachable::default(),
            budget: 8,
            climb: 1,
            flow: None,
            goal: None,
            wolves: Vec::new(),
            reservations: Reservations::new(),
            t_search_us: 0.0,
        }
    }

    fn spawn_wolves(&mut self) {
        let mut rng = Rng::new(5);
        let mut land: Vec<Hex> = self.world.hexes().filter(|&h| !self.world.is_submerged(h)).collect();
        land.sort();
        rng.shuffle(&mut land);
        self.reservations.clear();
        self.wolves = land
            .iter()
            .take(40)
            .enumerate()
            .map(|(i, &h)| {
                let mut w = Actor::new(UnitKind::Wolf, Team::Red, &self.world, h);
                w.mover.speed = rng.range_f32(1.6, 2.6);
                w.scale = 0.8;
                self.reservations.reserve(h, i as u32 + 1);
                w
            })
            .collect();
    }
}

impl Scene for PathDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        if common::common_update(ctx) {
            return Transition::Pop;
        }
        let dt = ctx.time.dt;
        let input = &ctx.input;
        for (i, m) in [Mode::AStar, Mode::Range, Mode::Flow].into_iter().enumerate() {
            if input.ui_clicked(&format!("mode{i}")) || input.key_pressed(Key::DIGITS[i + 1]) {
                self.mode = m;
                if m == Mode::Flow && self.wolves.is_empty() {
                    self.spawn_wolves();
                }
            }
        }
        if input.ui_clicked("budget+") {
            self.budget = (self.budget + 2).min(30);
        }
        if input.ui_clicked("budget-") {
            self.budget = self.budget.saturating_sub(2).max(2);
        }
        if input.ui_clicked("climb+") {
            self.climb = (self.climb + 1).min(4);
        }
        if input.ui_clicked("climb-") {
            self.climb = (self.climb - 1).max(0);
        }

        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        self.hover =
            if over_ui { None } else { self.world.pick(self.camera.screen_ray(ctx.input.mouse)).map(|p| p.hex) };

        let (world, mats, climb) = (&self.world, &self.mats, self.climb);
        let cost = |a: Hex, b: Hex| step_cost(world, mats, climb, a, b);
        let start = self.unit.hex();
        self.unit.climb = climb;
        let t0 = web_time::Instant::now();
        match self.mode {
            Mode::AStar => {
                self.path = self.hover.and_then(|g| path::astar(start, g, 1, u32::MAX, cost));
                if ctx.input.clicked(MouseButton::Left)
                    && !over_ui
                    && let Some(p) = &self.path
                {
                    let hexes = p.hexes.clone();
                    self.unit.walk_hexes(&hexes);
                }
            }
            Mode::Range => {
                self.range = path::reachable(start, self.budget, cost);
                if ctx.input.clicked(MouseButton::Left)
                    && !over_ui
                    && let Some(p) = self.hover.and_then(|h| self.range.path_to(h))
                {
                    self.unit.walk_hexes(&p);
                }
            }
            Mode::Flow => {
                if ctx.input.clicked(MouseButton::Left) && !over_ui {
                    self.goal = self.hover;
                }
                let goal = self.goal.unwrap_or(start);
                if self.flow.as_ref().is_none_or(|f| !f.is_goal(goal))
                    || ctx.input.ui_clicked("climb+")
                    || ctx.input.ui_clicked("climb-")
                {
                    self.flow = Some(FlowField::build([goal], u32::MAX, cost));
                }
                let ff = self.flow.as_ref().unwrap();
                let res = &mut self.reservations;
                for (i, w) in self.wolves.iter_mut().enumerate() {
                    let id = i as u32 + 1;
                    if !w.moving() {
                        let h = w.hex();
                        if let Some(n) = ff.next(h).filter(|&n| n != h) {
                            w.walk_hexes(&[n]);
                        }
                    }
                    // Terrain sets the speed; a wolf may only step into a hex it has reserved.
                    let events = w.update_with(world, &self.art, dt, |a, b| {
                        let cost = world.step_cost(a, b, climb)?;
                        res.reserve(b, id).then_some(cost)
                    });
                    res.track(id, &events);
                }
            }
        }
        self.t_search_us = self.t_search_us * 0.9 + t0.elapsed().as_secs_f32() * 1e6 * 0.1;
        self.unit.update(world, &self.art, dt);
        ctx.status = format!(
            "path mode={:?} cost={} range={} flow={} wolves={}",
            self.mode,
            self.path.as_ref().map_or(-1, |p| p.cost as i64),
            self.range.nodes.len(),
            self.flow.as_ref().map_or(0, |f| f.nodes.len()),
            self.wolves.len()
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        {
            let mut w = f.world(&mut self.world, &self.camera);
            for d in &self.decor {
                d.draw(&mut w);
            }
            match self.mode {
                Mode::AStar => {
                    if let Some(p) = &self.path {
                        w.path(&p.hexes, 0.12, Color::hex(0xffe27a).with_alpha(0.9), true);
                        if let Some(&last) = p.hexes.last() {
                            let top = w.world.hex_to_world(last);
                            w.label(top, &format!("cost {}", p.cost), 8.0, Color::WHITE);
                        }
                    } else if let Some(h) = self.hover {
                        w.hex_outline(h, Color::RED, 0.12);
                    }
                }
                Mode::Range => {
                    for (h, &(_, c)) in self.range.nodes.iter() {
                        let t = c as f32 / self.budget.max(1) as f32;
                        w.hex_fill_inset(h, Color::hex(0x5ad16a).lerp(Color::hex(0xf0c65a), t).with_alpha(0.35), 0.06);
                    }
                    if let Some(p) = self.hover.and_then(|h| self.range.path_to(h)) {
                        w.path(&p, 0.1, Color::WHITE.with_alpha(0.9), true);
                    }
                }
                Mode::Flow => {
                    if let Some(ff) = &self.flow {
                        let cam = w.camera.clone();
                        for (h, &(n, _)) in ff.nodes.iter() {
                            if h == n {
                                w.hex_fill(h, Color::hex(0xff5a5a).with_alpha(0.5));
                                continue;
                            }
                            let a = w.world.hex_to_world(h);
                            if !cam.sees_box(a - Vec3::ONE, a + Vec3::ONE) {
                                continue;
                            }
                            let b = w.world.hex_to_world(n);
                            let tip = a + (b - a) * 0.45;
                            w.line(a, tip, 0.07, Color::WHITE.with_alpha(0.55));
                            w.ring(tip, 0.08, Color::WHITE.with_alpha(0.7));
                        }
                    }
                    for wolf in &self.wolves {
                        wolf.draw(&mut w, &self.art, false);
                    }
                }
            }
            self.unit.draw(&mut w, &self.art, false);
        }
        common::header(f, "Pathfinding", "Step costs: terrain type + climbing; blocked by water and cliffs");
        common::stats(f);
        let help: &[&str] = match self.mode {
            Mode::AStar => &[
                "Hover: A* path from the pikeman   Click: walk there",
                "Road 1, grass 2, sand/rock 3, forest 4, snow 5; +50% per level up",
            ],
            Mode::Range => {
                &["Tinted: hexes reachable this turn (green cheap -> yellow)", "Click a tinted hex to walk there"]
            }
            Mode::Flow => &[
                "Click: set the goal. One flow field steers every wolf.",
                "Arrows point to the next hex on the cheapest route.",
            ],
        };
        let mut lines = help.to_vec();
        lines.push("1-3: modes   Right drag: pan   Wheel: zoom   Esc: menu");
        common::help(f, &lines);

        let size = f.ui_size();
        let r = Rect::new(size.x - 208.0, 106.0, 200.0, 190.0);
        f.panel(r);
        let dim = f.theme.text_dim;
        for (i, (m, name)) in
            [(Mode::AStar, "1 A* path"), (Mode::Range, "2 Move range"), (Mode::Flow, "3 Flow field")].iter().enumerate()
        {
            f.button_ex(
                &format!("mode{i}"),
                Rect::new(r.x + 8.0, r.y + 8.0 + i as f32 * 30.0, r.w - 16.0, 24.0),
                name,
                true,
                self.mode == *m,
            );
        }
        f.text(Vec2::new(r.x + 10.0, r.y + 102.0), &format!("MOVE POINTS  {}", self.budget), 8.0, dim);
        let c = Rect::new(r.x + 8.0, r.y + 114.0, r.w - 16.0, 22.0).cols(2, 4.0);
        f.button("budget-", c[0], "-");
        f.button("budget+", c[1], "+");
        f.text(Vec2::new(r.x + 10.0, r.y + 144.0), &format!("CLIMB LIMIT  {} level(s)", self.climb), 8.0, dim);
        let c = Rect::new(r.x + 8.0, r.y + 156.0, r.w - 16.0, 22.0).cols(2, 4.0);
        f.button("climb-", c[0], "-");
        f.button("climb+", c[1], "+");
        let t = format!("search {:.0} us", self.t_search_us);
        f.text(Vec2::new(r.x + 10.0, r.y + r.h + 6.0), &t, 8.0, dim);
    }
}
