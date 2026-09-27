//! Online play: connect to a relay (`cargo run -p knight-relay`), join a room and meet everyone
//! else in it. Each client moves its own hero (hex routes go over the wire), chats and emotes;
//! bubbles and emotes appear over the right heads for everyone. "Practice with a bot" runs the
//! same protocol through the in-process [`net::LocalHub`] transport, no server needed.

use std::collections::BTreeMap;
use std::rc::Rc;

use knight_engine::glam::{Vec2, Vec3};
use knight_engine::hex::path;
use knight_engine::hex::{Hex, Rng, shapes};
use knight_engine::net::{self, Client, ConnState, LocalHub, NetEvent, PeerId, Reader, Wire, Writer};
use knight_engine::*;

use crate::art::{Art, Team, UnitKind};
use crate::common::{self, Actor};

const KINDS: [UnitKind; 6] =
    [UnitKind::Paladin, UnitKind::Knight, UnitKind::Archer, UnitKind::Footman, UnitKind::Griffon, UnitKind::Ogre];

/// The game protocol (inside relay payloads).
#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Hello { name: String, kind: u8, at: Hex, route: Vec<Hex> },
    Route { from: Hex, hexes: Vec<Hex> },
    Chat { text: String },
    Emote { index: u8 },
    Ping { t: u32 },
    Pong { t: u32 },
}

fn put_hex(w: &mut Writer, h: Hex) {
    w.ivar(h.q as i64).ivar(h.r as i64);
}

fn get_hex(r: &mut Reader) -> net::codec::Result<Hex> {
    Ok(Hex::new(r.ivar()? as i32, r.ivar()? as i32))
}

fn put_hexes(w: &mut Writer, v: &[Hex]) {
    w.var(v.len() as u64);
    for &h in v {
        put_hex(w, h);
    }
}

fn get_hexes(r: &mut Reader) -> net::codec::Result<Vec<Hex>> {
    let n = r.var()? as usize;
    if n > 512 {
        return Err(net::DecodeError("route too long"));
    }
    (0..n).map(|_| get_hex(r)).collect()
}

impl Wire for Msg {
    fn encode(&self, w: &mut Writer) {
        match self {
            Msg::Hello { name, kind, at, route } => {
                w.u8(0).str(name).u8(*kind);
                put_hex(w, *at);
                put_hexes(w, route);
            }
            Msg::Route { from, hexes } => {
                w.u8(1);
                put_hex(w, *from);
                put_hexes(w, hexes);
            }
            Msg::Chat { text } => {
                w.u8(2).str(text);
            }
            Msg::Emote { index } => {
                w.u8(3).u8(*index);
            }
            Msg::Ping { t } => {
                w.u8(4).u32(*t);
            }
            Msg::Pong { t } => {
                w.u8(5).u32(*t);
            }
        }
    }

    fn decode(r: &mut Reader) -> net::codec::Result<Self> {
        Ok(match r.u8()? {
            0 => Msg::Hello { name: r.str()?, kind: r.u8()?, at: get_hex(r)?, route: get_hexes(r)? },
            1 => Msg::Route { from: get_hex(r)?, hexes: get_hexes(r)? },
            2 => Msg::Chat { text: r.str()? },
            3 => Msg::Emote { index: r.u8()? },
            4 => Msg::Ping { t: r.u32()? },
            5 => Msg::Pong { t: r.u32()? },
            _ => return Err(net::DecodeError("unknown message")),
        })
    }
}

struct Player {
    name: String,
    actor: Actor,
    rtt: Option<f32>,
}

/// A simulated remote player for offline practice.
struct Bot {
    client: Client,
    hex: Hex,
    timer: f32,
    rng: Rng,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Url,
    Room,
    Name,
}

pub struct OnlineDemo {
    art: Rc<Art>,
    world: HexWorld,
    camera: Camera,
    controller: CameraController,
    // Lobby.
    url: String,
    room: String,
    name: String,
    focus: Option<Field>,
    // Session.
    client: Option<Client>,
    hub: Option<LocalHub>,
    bot: Option<Bot>,
    players: BTreeMap<PeerId, Player>,
    speech: Speech,
    typing: bool,
    draft: String,
    error: String,
    ping_timer: f32,
    clock: f32,
    rng: Rng,
    sent: u32,
    received: u32,
}

impl OnlineDemo {
    pub fn new(art: Rc<Art>) -> Self {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        let mats = art.materials(&mut world);
        world.water = None;
        world.base_height = -2;
        for h in shapes::hexagon(Hex::ORIGIN, 9) {
            let (height, mat) = match h.length() {
                0..=1 => (1, mats.road),
                9 => (2, mats.rock),
                d if d % 3 == 0 => (0, mats.road),
                _ => (0, if (h.q * 7 + h.r * 3).rem_euclid(5) == 0 { mats.meadow } else { mats.grass }),
            };
            world.set_tile(h, Tile::new(height, mat));
        }
        let host = knight_engine::web_host().unwrap_or_else(|| "127.0.0.1".into());
        let mut rng = Rng::new(
            (knight_engine::web_time::SystemTime::now()
                .duration_since(knight_engine::web_time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(7)) as u64,
        );
        let name = format!("Hero{}", rng.range(100, 999));
        OnlineDemo {
            art,
            world,
            camera: Camera::new(Vec3::ZERO, 44.0).with_pitch(48.0),
            controller: CameraController::default().drag_with(&[MouseButton::Right, MouseButton::Middle]),
            url: format!("ws://{host}:9001"),
            room: "plaza".into(),
            name,
            focus: None,
            client: None,
            hub: None,
            bot: None,
            players: BTreeMap::new(),
            speech: Speech::new(SpeechConfig::default()),
            typing: false,
            draft: String::new(),
            error: String::new(),
            ping_timer: 0.0,
            clock: 0.0,
            rng,
            sent: 0,
            received: 0,
        }
    }

    fn me(&self) -> Option<PeerId> {
        self.client.as_ref().and_then(|c| c.me())
    }

    fn send(&mut self, m: &Msg) {
        if let Some(c) = &mut self.client {
            c.broadcast_msg(m);
            self.sent += 1;
        }
    }

    fn spawn_hex(&self, id: PeerId) -> Hex {
        let ring = Hex::ORIGIN.ring(3 + (id as i32 % 4));
        ring[(id as usize * 7) % ring.len()]
    }

    fn add_player(&mut self, id: PeerId, name: String, kind: u8, at: Hex) {
        let kind = KINDS[kind as usize % KINDS.len()];
        let team = if Some(id) == self.me() { Team::Blue } else { Team::Red };
        let mut actor = Actor::new(kind, team, &self.world, at);
        actor.mover.speed = 2.2;
        self.players.insert(id, Player { name, actor, rtt: None });
    }

    fn hello(&self) -> Msg {
        let me = self.me().unwrap_or(0);
        let p = self.players.get(&me);
        let at = p.map_or(self.spawn_hex(me), |p| p.actor.mover.next_stop());
        let route = p.map_or(Vec::new(), |p| p.actor.mover.route().collect());
        Msg::Hello { name: self.name.clone(), kind: (me % KINDS.len() as u32) as u8, at, route }
    }

    fn connect(&mut self, offline: bool) {
        self.disconnect();
        self.error.clear();
        if offline {
            let hub = LocalHub::new();
            let client = hub.connect(&self.room, &self.name);
            let bot = Bot {
                client: hub.connect(&self.room, "Bot Bertha"),
                hex: Hex::new(2, 1),
                timer: 1.0,
                rng: Rng::new(3),
            };
            self.client = Some(client);
            self.bot = Some(bot);
            self.hub = Some(hub);
        } else {
            self.client = Some(net::connect_ws(&self.url, &self.room, &self.name));
        }
    }

    fn disconnect(&mut self) {
        self.client = None;
        self.bot = None;
        self.hub = None;
        self.players.clear();
        self.speech = Speech::new(self.speech.config.clone());
    }

    fn handle(&mut self, from: PeerId, m: Msg) {
        self.received += 1;
        match m {
            Msg::Hello { name, kind, at, route } => {
                if !self.players.contains_key(&from) {
                    self.add_player(from, name, kind, at);
                    if let Some(p) = self.players.get_mut(&from) {
                        let _ = p.actor.mover.set_route(&route);
                    }
                }
            }
            Msg::Route { from: start, hexes } => {
                if let Some(p) = self.players.get_mut(&from) {
                    if p.actor.mover.next_stop() != start {
                        p.actor.place(&self.world, start);
                    }
                    let _ = p.actor.mover.set_route(&hexes);
                }
            }
            Msg::Chat { text } => {
                let name = self.players.get(&from).map_or("?".into(), |p| p.name.clone());
                self.speech.say(from as u64, &name, &text, Priority::Normal);
            }
            Msg::Emote { index } => {
                if let Some(&(_, img)) = self.art.emotes.get(index as usize) {
                    self.speech.emote(from as u64, img);
                }
            }
            Msg::Ping { t } => {
                if let Some(c) = &mut self.client {
                    c.send_to(from, Msg::Pong { t }.to_bytes());
                }
            }
            Msg::Pong { t } => {
                let now = (self.clock * 1000.0) as u32;
                if let Some(p) = self.players.get_mut(&from) {
                    p.rtt = Some(now.saturating_sub(t) as f32);
                }
            }
        }
    }

    fn emote(&mut self, index: usize) {
        if let (Some(me), Some(&(_, img))) = (self.me(), self.art.emotes.get(index)) {
            self.speech.emote(me as u64, img);
            self.send(&Msg::Emote { index: index as u8 });
        }
    }

    fn step_bot(&mut self, dt: f32) {
        let Some(bot) = &mut self.bot else { return };
        let world = &self.world;
        for e in bot.client.update() {
            if let NetEvent::Connected { .. } | NetEvent::PeerJoined { .. } = e {
                let m = Msg::Hello { name: "Bot Bertha".into(), kind: 4, at: bot.hex, route: Vec::new() };
                bot.client.broadcast_msg(&m);
            }
            if let NetEvent::Data { from, data } = e
                && let Ok(Msg::Ping { t }) = Msg::from_bytes(&data)
            {
                bot.client.send_to(from, Msg::Pong { t }.to_bytes());
            }
        }
        bot.timer -= dt;
        if bot.timer <= 0.0 {
            bot.timer = bot.rng.range_f32(2.0, 4.0);
            match bot.rng.range(0, 4) {
                0 => {
                    let line = ["hello there!", "nice day for a walk", "follow me", "gg", "anyone want to trade?"];
                    let text = line[bot.rng.range(0, line.len() as i32) as usize].to_string();
                    bot.client.broadcast_msg(&Msg::Chat { text });
                }
                1 => {
                    let index = bot.rng.range(0, 8) as u8;
                    bot.client.broadcast_msg(&Msg::Emote { index });
                }
                _ => {
                    let goal = bot.hex + Hex::new(bot.rng.range(-3, 4), bot.rng.range(-3, 4));
                    if goal.length() <= 8
                        && let Some(p) = path::astar(bot.hex, goal, 1, 100, |a, b| world.step_points(a, b, 1, 2))
                    {
                        bot.client.broadcast_msg(&Msg::Route { from: bot.hex, hexes: p.hexes });
                        bot.hex = goal;
                    }
                }
            }
        }
    }
}

impl Scene for OnlineDemo {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        let dt = ctx.time.dt;
        self.clock += dt;
        let editing = self.typing || self.focus.is_some();
        if !editing {
            if common::common_update(ctx) {
                return Transition::Pop;
            }
            if self.client.is_some() {
                if ctx.input.key_pressed(Key::Enter) {
                    self.typing = true;
                }
                if let Some(d) = ctx.input.digit_pressed()
                    && (1..=8).contains(&d)
                {
                    self.emote(d - 1);
                }
            }
        }
        if ctx.input.ui_clicked("connect") {
            self.connect(false);
        }
        if ctx.input.ui_clicked("offline") {
            self.connect(true);
        }
        if ctx.input.ui_clicked("leave") {
            self.disconnect();
        }
        for k in 0..8 {
            if ctx.input.ui_clicked(&format!("emote{k}")) {
                self.emote(k);
            }
        }

        // Network.
        self.step_bot(dt);
        let events = self.client.as_mut().map(|c| c.update()).unwrap_or_default();
        for e in events {
            match e {
                NetEvent::Connected { you } => {
                    let at = self.spawn_hex(you);
                    let name = self.name.clone();
                    self.add_player(you, name, (you % KINDS.len() as u32) as u8, at);
                    let hello = self.hello();
                    self.send(&hello);
                    ctx.audio.play(self.art.sfx.powerup);
                }
                NetEvent::PeerJoined { id, name } => {
                    // Introduce ourselves to the newcomer (they will send their Hello).
                    let hello = self.hello();
                    if let Some(c) = &mut self.client {
                        c.send_to(id, hello.to_bytes());
                    }
                    self.speech.say(id as u64, &name, "joined", Priority::Normal);
                    ctx.audio.play(self.art.sfx.pop);
                }
                NetEvent::PeerLeft { id } => {
                    self.players.remove(&id);
                    self.speech.remove(id as u64);
                }
                NetEvent::Data { from, data } => match Msg::from_bytes(&data) {
                    Ok(m) => {
                        if matches!(m, Msg::Chat { .. } | Msg::Emote { .. }) {
                            ctx.audio.play(self.art.sfx.pop);
                        }
                        self.handle(from, m);
                    }
                    Err(e) => log::warn!("bad message from {from}: {e}"),
                },
                NetEvent::Disconnected { reason } => {
                    self.error = reason;
                    self.client = None;
                    self.players.clear();
                }
            }
        }
        self.ping_timer -= dt;
        if self.ping_timer <= 0.0 && self.me().is_some() {
            self.ping_timer = 1.0;
            let t = (self.clock * 1000.0) as u32;
            self.send(&Msg::Ping { t });
        }

        // Your hero: click a hex to walk there; the route goes to everyone.
        common::sync_camera(ctx, &mut self.camera, self.world.pixels_per_unit);
        let over_ui = ctx.input.pointer_over_ui();
        self.controller.keyboard = !editing;
        self.controller.update(&mut self.camera, &ctx.input, dt, over_ui);
        if let Some(me) = self.me()
            && ctx.input.clicked(MouseButton::Left)
            && !over_ui
            && let Some(p) = self.world.pick(self.camera.screen_ray(ctx.input.mouse))
            && let Some(player) = self.players.get_mut(&me)
        {
            let from = player.actor.mover.next_stop();
            let world = &self.world;
            if let Some(route) = path::astar(from, p.hex, 1, 200, |a, b| world.step_points(a, b, 1, 2)) {
                player.actor.walk_hexes(&route.hexes);
                self.send(&Msg::Route { from, hexes: route.hexes });
            }
        }
        for p in self.players.values_mut() {
            p.actor.update(&self.world, &self.art, dt);
        }
        if let Some(me) = self.me().and_then(|m| self.players.get(&m)) {
            self.camera.move_to(me.actor.pos);
        }
        self.speech.update(dt);
        let state = match self.client.as_ref().map(|c| c.state()) {
            Some(ConnState::Connected) => "connected",
            Some(ConnState::Connecting) => "connecting",
            _ => "offline",
        };
        ctx.status = format!(
            "online state={state} me={:?} players={} sent={} received={} bubbles={}",
            self.me(),
            self.players.len(),
            self.sent,
            self.received,
            self.speech.active_bubbles()
        );
        let _ = &mut self.rng;
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let me = self.me();
        {
            let mut w = f.world(&mut self.world, &self.camera);
            w.sprite(
                Sprite::new(self.art.keep_neutral, Vec3::new(0.0, w.world.surface_y(Hex::ORIGIN), 0.0)).scale(1.4),
            );
            let up = w.camera.basis().1;
            for (id, p) in &mut self.players {
                p.actor.selected = Some(*id) == me;
                p.actor.draw(&mut w, &self.art, false);
                let col = if Some(*id) == me { Color::hex(0xffe27a) } else { Color::hex(0xd8d0c0) };
                w.label(p.actor.pos + Vec3::Z * 0.55, &p.name, 8.0, col);
            }
            let heads: BTreeMap<u64, Vec3> =
                self.players.iter().map(|(id, p)| (*id as u64, p.actor.pos + up * 1.75)).collect();
            let focus =
                me.and_then(|m| self.players.get(&m)).map_or(w.camera.viewport.center(), |p| w.to_screen(p.actor.pos));
            self.speech.draw(&mut w, focus, |id| heads.get(&id).copied());
        }
        common::header(f, "Online Play", "Relay rooms over WebSocket: routes, chat and emotes shared by everyone");
        common::stats(f);
        let size = f.ui_size();
        let t = f.theme.clone();

        if self.client.is_none() {
            // Lobby.
            let r = Rect::new(size.x * 0.5 - 230.0, size.y * 0.5 - 150.0, 460.0, 300.0);
            f.panel(r);
            f.text(Vec2::new(r.x + 16.0, r.y + 14.0), "JOIN A ROOM", 16.0, t.accent);
            let mut y = r.y + 50.0;
            for (label, field) in [("Relay", Field::Url), ("Room", Field::Room), ("Name", Field::Name)] {
                f.text(Vec2::new(r.x + 16.0, y + 8.0), label, 8.0, t.text_dim);
                let mut focused = self.focus == Some(field);
                let value = match field {
                    Field::Url => &mut self.url,
                    Field::Room => &mut self.room,
                    Field::Name => &mut self.name,
                };
                let mut v = std::mem::take(value);
                f.text_field(label, Rect::new(r.x + 80.0, y, r.w - 96.0, 24.0), &mut v, &mut focused, "");
                *match field {
                    Field::Url => &mut self.url,
                    Field::Room => &mut self.room,
                    Field::Name => &mut self.name,
                } = v;
                if focused {
                    self.focus = Some(field);
                } else if self.focus == Some(field) {
                    self.focus = None;
                }
                y += 34.0;
            }
            f.button("connect", Rect::new(r.x + 16.0, y + 6.0, 200.0, 40.0), "Connect");
            f.button("offline", Rect::new(r.x + r.w - 216.0, y + 6.0, 200.0, 40.0), "Practice with a bot");
            let hint = "Start a relay: cargo run -p knight-relay --release\nthen open this page in two tabs and join the same room.";
            f.text(Vec2::new(r.x + 16.0, y + 62.0), hint, 8.0, t.text_dim);
            if !self.error.is_empty() {
                f.text_wrapped(
                    Rect::new(r.x + 16.0, r.y + r.h - 34.0, r.w - 32.0, 30.0),
                    &self.error,
                    8.0,
                    Color::hex(0xff8a7a),
                );
            }
            return;
        }

        // Room panel: who is here and their ping.
        let pr = Rect::new(size.x - 208.0, 106.0, 200.0, 40.0 + self.players.len() as f32 * 14.0 + 30.0);
        f.panel(pr);
        let state = match self.client.as_ref().map(|c| c.state()) {
            Some(ConnState::Connected) => "CONNECTED",
            _ => "CONNECTING...",
        };
        f.text(Vec2::new(pr.x + 10.0, pr.y + 8.0), &format!("{state}  room '{}'", self.room), 8.0, t.accent);
        for (k, (id, p)) in self.players.iter().enumerate() {
            let y = pr.y + 24.0 + k as f32 * 14.0;
            let col = if Some(*id) == me { t.accent } else { t.text };
            f.text(Vec2::new(pr.x + 10.0, y), &p.name, 8.0, col);
            let ping =
                if Some(*id) == me { "you".to_string() } else { p.rtt.map_or("...".into(), |r| format!("{r:.0} ms")) };
            f.text(Vec2::new(pr.x + 130.0, y), &ping, 8.0, t.text_dim);
        }
        f.button("leave", Rect::new(pr.x + 8.0, pr.y + pr.h - 28.0, pr.w - 16.0, 22.0), "Leave");

        // Chat field + emote bar.
        let field = Rect::new(8.0, size.y - 100.0, 420.0, 24.0);
        let mut typing = self.typing;
        let mut draft = std::mem::take(&mut self.draft);
        if f.text_field("chat", field, &mut draft, &mut typing, "Enter to chat") {
            let text = std::mem::take(&mut draft);
            if let Some(me) = me {
                self.speech.say(me as u64, &self.name.clone(), &text, Priority::Me);
            }
            self.send(&Msg::Chat { text });
            typing = false;
        }
        self.draft = draft;
        self.typing = typing;
        let bar = Rect::new(8.0, size.y - 68.0, 392.0, 48.0);
        f.panel(bar);
        for (k, (_, img)) in self.art.emotes.iter().enumerate() {
            let cell = Rect::new(bar.x + 8.0 + k as f32 * 47.0, bar.y + 6.0, 40.0, 36.0);
            f.button(&format!("emote{k}"), cell, "");
            f.image_fit(cell.inset(4.0), *img, Color::WHITE);
            f.text(Vec2::new(cell.x + 2.0, cell.y + 2.0), &format!("{}", k + 1), 8.0, t.text_dim);
        }
        f.text(
            Vec2::new(8.0, size.y - 14.0),
            &format!("sent {}  received {}", self.sent, self.received),
            8.0,
            t.text_dim,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_roundtrip() {
        for m in [
            Msg::Hello {
                name: "a".into(),
                kind: 2,
                at: Hex::new(-3, 4),
                route: vec![Hex::new(-3, 4), Hex::new(-2, 4)],
            },
            Msg::Route { from: Hex::new(1, -1), hexes: vec![Hex::new(1, -1), Hex::new(2, -1)] },
            Msg::Chat { text: "hi there".into() },
            Msg::Emote { index: 7 },
            Msg::Ping { t: 12345 },
        ] {
            assert_eq!(Msg::from_bytes(&m.to_bytes()).unwrap(), m);
        }
    }

    /// Two demo instances on one in-process hub see each other move and chat.
    #[test]
    fn two_clients_share_moves_and_chat() {
        let mut ctx = Context::new();
        let art = Art::load(&mut ctx);
        let hub = LocalHub::new();
        let mut a = OnlineDemo::new(art.clone());
        let mut b = OnlineDemo::new(art);
        a.client = Some(hub.connect("r", "A"));
        b.client = Some(hub.connect("r", "B"));
        ctx.time.dt = 1.0 / 30.0;
        for _ in 0..10 {
            a.update(&mut ctx);
            b.update(&mut ctx);
        }
        assert_eq!(a.players.len(), 2);
        assert_eq!(b.players.len(), 2);
        let (ia, ib) = (a.me().unwrap(), b.me().unwrap());
        // A walks; B sees the route.
        let from = a.players[&ia].actor.mover.next_stop();
        let to = from.neighbor(0);
        a.players.get_mut(&ia).unwrap().actor.walk_hexes(&[from, to]);
        a.send(&Msg::Route { from, hexes: vec![from, to] });
        a.send(&Msg::Chat { text: "hello B".into() });
        for _ in 0..60 {
            a.update(&mut ctx);
            b.update(&mut ctx);
        }
        assert_eq!(b.players[&ia].actor.hex(), to);
        assert!(b.speech.log().any(|l| l.text == "hello B"));
        assert!(a.players[&ib].rtt.is_some(), "ping measured");
    }
}
