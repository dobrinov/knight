//! Chat & emotes: a crowded plaza of (simulated) players who walk around and talk. Shows how
//! the engine keeps chat readable in crowds: one bubble per speaker, a budget of visible bubbles
//! ranked by priority (you, your party, then strangers), "..." markers for everyone else,
//! repeated lines collapsed to "x3", bubbles that never overlap, and identical emotes from a
//! crowd merged into one icon with a count. Type with Enter; emote with the bar or /commands.

use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::{Hex, Rng, shapes};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common;
use crate::realtime::{Order, RtWorld};

const ME: usize = 0;
const PARTY: usize = 4;

const LINES: [&str; 24] = [
    "hi!",
    "anyone up for the dungeon?",
    "LFG raid, need a healer",
    "wts shiny sword 500g",
    "lol",
    "where is the blacksmith?",
    "gg",
    "brb",
    "this town is so pretty at night",
    "who wants to duel?",
    "anyone seen the dragon today",
    "selling potions, cheap cheap cheap!",
    "follow me",
    "thanks!",
    "wow",
    "does anyone know how to get to the eastern keep past the mountains?",
    "ok",
    "nice armor",
    "guild recruiting, pst",
    "hello",
    "on my way",
    "haha",
    "can someone help me with a quest",
    "afk",
];

const NAMES: [&str; 16] = [
    "Aria", "Bram", "Cora", "Dain", "Edda", "Finn", "Gwen", "Hale", "Iris", "Jory", "Kael", "Lina", "Moss", "Nell",
    "Orin", "Pip",
];

pub struct ChatDemo {
    art: Rc<Art>,
    world: HexWorld,
    camera: Camera,
    controller: CameraController,
    rt: RtWorld,
    speech: Speech,
    names: Vec<String>,
    rng: Rng,
    crowd: usize,
    chatter: f32,
    typing: bool,
    draft: String,
    show_names: bool,
    wander_timer: f32,
    /// Sounds to play for your own messages and emotes.
    pops: u32,
}

impl ChatDemo {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        world.water = None;
        world.base_height = -2;
        for h in shapes::hexagon(Hex::ORIGIN, 12) {
            let (height, mat) = match h.length() {
                0..=2 => (1, mats.road),
                12 => (2, mats.rock),
                d if d % 4 == 0 || h.q == 0 || h.r == 0 => (0, mats.road),
                _ => (0, mats.grass),
            };
            world.set_tile(h, Tile::new(height, mat));
        }
        let camera = Camera::new(Vec3::ZERO, 44.0).with_pitch(48.0);
        let mut demo = ChatDemo {
            art,
            world,
            camera,
            controller: CameraController::default().drag_with(&[MouseButton::Right, MouseButton::Middle]),
            rt: RtWorld::new(),
            speech: Speech::new(SpeechConfig::default()),
            names: Vec::new(),
            rng: Rng::new(12),
            crowd: 40,
            chatter: 1.0,
            typing: false,
            draft: String::new(),
            show_names: true,
            wander_timer: 0.0,
            pops: 0,
        };
        demo.populate();
        demo
    }

    fn populate(&mut self) {
        self.rt = RtWorld::new();
        self.names.clear();
        let spots: Vec<Hex> = shapes::hexagon(Hex::ORIGIN, 10).into_iter().filter(|h| h.length() > 3).collect();
        let kinds = [UnitKind::Footman, UnitKind::Archer, UnitKind::Knight, UnitKind::Ogre, UnitKind::Griffon];
        self.rt.spawn(&self.world, UnitKind::Paladin, Team::Blue, Hex::new(0, 3));
        self.names.push("You".into());
        while self.rt.units.len() < self.crowd + 1 {
            let i = self.rt.units.len();
            let team = if i <= PARTY { Team::Blue } else { Team::Red };
            let h = spots[self.rng.range(0, spots.len() as i32) as usize];
            let kind = kinds[self.rng.range(0, kinds.len() as i32) as usize];
            if self.rt.spawn(&self.world, kind, team, h).is_some() {
                let n = NAMES[self.rng.range(0, NAMES.len() as i32) as usize];
                self.names.push(format!("{n}{}", self.rng.range(1, 99)));
            }
        }
        for u in &mut self.rt.units {
            u.aggro = 0;
            u.actor.mover.speed = 1.4;
        }
        self.speech = Speech::new(self.speech.config.clone());
    }

    fn priority(i: usize) -> Priority {
        match i {
            ME => Priority::Me,
            1..=PARTY => Priority::Friend,
            _ => Priority::Normal,
        }
    }

    fn emote(&mut self, speaker: usize, name: &str) {
        if speaker == ME {
            self.pops += 1;
        }
        if let Some(&(_, img)) = self.art.emotes.iter().find(|(n, _)| *n == name) {
            self.speech.emote(speaker as u64, img);
        }
    }

    fn send(&mut self) {
        let text = std::mem::take(&mut self.draft);
        let text = text.trim();
        if let Some(cmd) = text.strip_prefix('/') {
            self.emote(ME, cmd);
        } else {
            self.speech.say(ME as u64, "You", text, Priority::Me);
            self.pops += 1;
        }
    }
}

impl Scene for ChatDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        let dt = ctx.time.dt;
        if !self.typing {
            if common::common_update(ctx) {
                return Transition::Pop;
            }
            if ctx.input.key_pressed(Key::Enter) {
                self.typing = true;
            }
            // 1-8: emotes.
            if let Some(d) = ctx.input.digit_pressed()
                && (1..=self.art.emotes.len()).contains(&d)
            {
                let name = self.art.emotes[d - 1].0;
                self.emote(ME, name);
            }
        }
        for (k, (name, _)) in self.art.emotes.clone().iter().enumerate() {
            if ctx.input.ui_clicked(&format!("emote{k}")) {
                self.emote(ME, name);
            }
        }
        if ctx.input.ui_clicked("cheer") {
            // Everyone cheers: the icons merge into one with a count.
            for i in 0..self.rt.units.len() {
                self.emote(i, "cheer");
            }
        }
        for (id, n) in [("crowd15", 15), ("crowd40", 40), ("crowd100", 100)] {
            if ctx.input.ui_clicked(id) {
                self.crowd = n;
                self.populate();
            }
        }
        for (id, c) in [("quiet", 0.3), ("busy", 1.0), ("riot", 4.0)] {
            if ctx.input.ui_clicked(id) {
                self.chatter = c;
            }
        }
        for (id, n) in [("max3", 3), ("max6", 6), ("max12", 12)] {
            if ctx.input.ui_clicked(id) {
                self.speech.config.max_bubbles = n;
            }
        }
        if ctx.input.ui_clicked("names") {
            self.show_names = !self.show_names;
        }

        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.keyboard = !self.typing;
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        if ctx.input.clicked(MouseButton::Left)
            && !over_ui
            && let Some(p) = self.world.pick(self.camera.screen_ray(ctx.input.mouse))
        {
            self.rt.order_move(&self.world, &[ME], p.hex);
        }

        // The simulated crowd wanders and talks.
        self.wander_timer -= dt;
        if self.wander_timer <= 0.0 {
            self.wander_timer = 0.5;
            for i in 1..self.rt.units.len() {
                if self.rt.units[i].order == Order::Idle && self.rng.chance(0.15) {
                    let goal = self.rt.units[i].hex() + Hex::new(self.rng.range(-3, 4), self.rng.range(-3, 4));
                    if goal.length() <= 10 {
                        self.rt.order_move(&self.world, &[i], goal);
                    }
                }
            }
        }
        let n = self.rt.units.len();
        let talks = (dt * self.chatter * n as f32 * 0.12).floor() as usize
            + usize::from(self.rng.chance((dt * self.chatter * n as f32 * 0.12).fract()));
        for _ in 0..talks {
            let i = self.rng.range(1, n as i32) as usize;
            if self.rng.chance(0.2) {
                let e = self.rng.range(0, self.art.emotes.len() as i32) as usize;
                let name = self.art.emotes[e].0;
                self.emote(i, name);
            } else {
                let line = LINES[self.rng.range(0, LINES.len() as i32) as usize];
                let name = self.names[i].clone();
                self.speech.say(i as u64, &name, line, Self::priority(i));
            }
        }
        self.rt.step(&self.world, &self.art, dt);
        self.speech.update(dt);
        for _ in 0..std::mem::take(&mut self.pops).min(3) {
            ctx.audio.play(self.art.sfx.pop);
        }
        ctx.status = format!(
            "chat crowd={} bubbles={} hidden={} log={} typing={}",
            self.rt.units.len(),
            self.speech.active_bubbles(),
            self.speech.hidden_last_frame,
            self.speech.log().count(),
            self.typing
        );
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let me = self.rt.units[ME].actor.pos;
        {
            let mut w = f.world(&mut self.world, &self.camera);
            w.sprite(Sprite::new(self.art.keep[0], Vec3::new(0.0, w.world.surface_y(Hex::ORIGIN), 0.0)).scale(1.6));
            for (i, u) in self.rt.units.iter_mut().enumerate() {
                u.actor.selected = i == ME;
                u.actor.draw(&mut w, &self.art, false);
            }
            let up = w.camera.basis().1;
            if self.show_names {
                for (i, u) in self.rt.units.iter().enumerate() {
                    let col = match Self::priority(i) {
                        Priority::Me => Color::hex(0xffe27a),
                        Priority::Friend => Color::hex(0x9fd8ff),
                        Priority::Normal => Color::hex(0xd8d0c0),
                    };
                    w.label(u.actor.pos + Vec3::Z * 0.55, &self.names[i], 8.0, col.with_alpha(0.85));
                }
            }
            let focus = w.to_screen(me);
            let heads: Vec<Vec3> = self.rt.units.iter().map(|u| u.actor.pos + up * (1.75 * u.actor.scale)).collect();
            self.speech.draw(&mut w, focus, |id| heads.get(id as usize).copied());
        }
        common::header(
            f,
            "Chat & Emotes",
            "Readable chat in crowds: bubble budget, priorities, markers, merged emotes",
        );
        common::stats(f);

        // Chat log + input.
        let size = f.ui_size();
        let t = f.theme.clone();
        let log = Rect::new(8.0, size.y - 170.0, 420.0, 162.0);
        f.panel(log);
        let lines: Vec<_> = self.speech.log().rev().take(9).collect::<Vec<_>>().into_iter().rev().cloned().collect();
        for (k, l) in lines.iter().enumerate() {
            let col = match Self::priority(l.speaker as usize) {
                Priority::Me => t.accent,
                Priority::Friend => Color::hex(0x9fd8ff),
                Priority::Normal => t.text_dim,
            };
            let x = f.text(Vec2::new(log.x + 8.0, log.y + 8.0 + k as f32 * 13.0), &format!("{}:", l.name), 8.0, col);
            let room = ((log.w - 24.0 - x) / 8.0) as usize;
            let text: String = l.text.chars().take(room).collect();
            f.text(Vec2::new(log.x + 14.0 + x, log.y + 8.0 + k as f32 * 13.0), &text, 8.0, t.text);
        }
        let field = Rect::new(log.x + 6.0, log.y + log.h - 30.0, log.w - 12.0, 24.0);
        let mut typing = self.typing;
        let mut draft = std::mem::take(&mut self.draft);
        if f.text_field("chat", field, &mut draft, &mut typing, "Enter to chat, /love /laugh /cheer ... to emote") {
            self.draft = draft;
            self.send();
            typing = false;
        } else {
            self.draft = draft;
        }
        self.typing = typing;

        // Emote bar.
        let bar = Rect::new(size.x * 0.5 - 196.0, size.y - 56.0, 392.0, 48.0);
        f.panel(bar);
        for (k, (name, img)) in self.art.emotes.iter().enumerate() {
            let cell = Rect::new(bar.x + 8.0 + k as f32 * 47.0, bar.y + 6.0, 40.0, 36.0);
            f.button(&format!("emote{k}"), cell, "");
            f.image_fit(cell.inset(4.0), *img, Color::WHITE);
            f.text(Vec2::new(cell.x + 2.0, cell.y + 2.0), &format!("{}", k + 1), 8.0, t.text_dim);
            if f.hovered(cell) {
                f.tooltip(&format!("/{name}  (key {})", k + 1));
            }
        }

        // Options.
        let r = Rect::new(size.x - 208.0, 106.0, 200.0, 222.0);
        f.panel(r);
        let dim = t.text_dim;
        f.text(Vec2::new(r.x + 10.0, r.y + 8.0), "CROWD", 8.0, dim);
        let c = Rect::new(r.x + 8.0, r.y + 20.0, r.w - 16.0, 22.0).cols(3, 4.0);
        f.button_ex("crowd15", c[0], "15", true, self.crowd == 15);
        f.button_ex("crowd40", c[1], "40", true, self.crowd == 40);
        f.button_ex("crowd100", c[2], "100", true, self.crowd == 100);
        f.text(Vec2::new(r.x + 10.0, r.y + 50.0), "CHATTER", 8.0, dim);
        let c = Rect::new(r.x + 8.0, r.y + 62.0, r.w - 16.0, 22.0).cols(3, 4.0);
        f.button_ex("quiet", c[0], "Quiet", true, self.chatter < 0.5);
        f.button_ex("busy", c[1], "Busy", true, self.chatter == 1.0);
        f.button_ex("riot", c[2], "Riot", true, self.chatter > 2.0);
        f.text(Vec2::new(r.x + 10.0, r.y + 92.0), "MAX BUBBLES", 8.0, dim);
        let c = Rect::new(r.x + 8.0, r.y + 104.0, r.w - 16.0, 22.0).cols(3, 4.0);
        let mb = self.speech.config.max_bubbles;
        f.button_ex("max3", c[0], "3", true, mb == 3);
        f.button_ex("max6", c[1], "6", true, mb == 6);
        f.button_ex("max12", c[2], "12", true, mb == 12);
        f.button_ex("names", Rect::new(r.x + 8.0, r.y + 134.0, r.w - 16.0, 22.0), "Names", true, self.show_names);
        f.button("cheer", Rect::new(r.x + 8.0, r.y + 162.0, r.w - 16.0, 26.0), "Everyone cheers!");
        let info = format!("{} talking, {} as ...", self.speech.active_bubbles(), self.speech.hidden_last_frame);
        f.text(Vec2::new(r.x + 10.0, r.y + 200.0), &info, 8.0, dim);
    }
}
