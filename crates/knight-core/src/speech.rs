//! Chat bubbles and emotes over characters, built for crowded multiplayer scenes.
//!
//! Rules that keep a busy town square readable:
//! - **One bubble per speaker.** A new line replaces the old one; a repeated line becomes "×2".
//! - **A budget of visible bubbles** ([`SpeechConfig::max_bubbles`]). Speakers are ranked by
//!   priority (you, party, then everyone), recency and closeness to the focus point. Speakers
//!   over budget show a small "…" marker instead, so you can still see who is talking.
//! - **No overlap.** Bubbles are laid out in screen space; ones pushed away from their speaker
//!   get a tail pointing back at them.
//! - **Zoomed out**, text is replaced by markers; far away speakers are not drawn at all.
//! - **Emotes** are small icon pop-ups. The same emote from many nearby characters merges into
//!   one icon with a count (a stadium of hearts is one "♥ ×40").
//! - **Rate limits** per speaker, and every line also goes to a [`ChatLine`] log for a chat panel.

use std::collections::{HashMap, VecDeque};

use glam::{Vec2, Vec3};

use crate::assets::ImageId;
use crate::frame::WorldDraw;
use crate::{Color, Rect};

/// How important a speaker is to the local player.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    #[default]
    Normal,
    /// Party, guild, friends.
    Friend,
    /// The local player (always shown).
    Me,
}

#[derive(Clone, Debug)]
pub struct SpeechConfig {
    /// Most text bubbles shown at once.
    pub max_bubbles: usize,
    /// Wrap width in characters and maximum lines (longer text ends with "...").
    pub line_chars: usize,
    pub max_lines: usize,
    /// Bubble lifetime: base seconds plus seconds per character.
    pub base_time: f32,
    pub per_char: f32,
    /// Below this camera zoom (logical px per world unit) bubbles collapse to markers.
    pub min_zoom_for_text: f32,
    /// Minimum seconds between lines from one speaker (extra lines are dropped from bubbles
    /// but still logged).
    pub min_interval: f32,
    /// Emotes closer than this (logical px) with the same image merge into one.
    pub emote_merge_px: f32,
    /// Keep this many log lines.
    pub log_size: usize,
}

impl Default for SpeechConfig {
    fn default() -> Self {
        SpeechConfig {
            max_bubbles: 6,
            line_chars: 26,
            max_lines: 3,
            base_time: 2.5,
            per_char: 0.06,
            min_zoom_for_text: 22.0,
            min_interval: 0.4,
            emote_merge_px: 48.0,
            log_size: 60,
        }
    }
}

/// A line in the chat log.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatLine {
    pub speaker: u64,
    pub name: String,
    pub text: String,
    pub time: f64,
}

#[derive(Clone, Debug)]
struct Bubble {
    text: String,
    lines: Vec<String>,
    age: f32,
    life: f32,
    repeats: u32,
    priority: Priority,
    last_said: f64,
}

#[derive(Clone, Debug)]
struct Emote {
    speaker: u64,
    image: ImageId,
    age: f32,
}

const EMOTE_LIFE: f32 = 1.8;

/// Chat bubbles, emotes and the chat log for one world.
#[derive(Clone, Debug, Default)]
pub struct Speech {
    pub config: SpeechConfig,
    bubbles: HashMap<u64, Bubble>,
    emotes: Vec<Emote>,
    log: VecDeque<ChatLine>,
    time: f64,
    /// How many bubbles were over budget last frame (for UI / tests).
    pub hidden_last_frame: usize,
}

/// Word-wrap `text` into at most `max_lines` lines of `width` characters.
pub fn wrap(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    let width = width.max(4);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        while word.chars().count() > width {
            let head: String = word.chars().take(width).collect();
            word = word.chars().skip(width).collect();
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            lines.push(head);
        }
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.last_mut().unwrap();
        let keep: String = last.chars().take(width.saturating_sub(3)).collect();
        *last = format!("{keep}...");
    }
    lines
}

impl Speech {
    pub fn new(config: SpeechConfig) -> Self {
        Speech { config, ..Default::default() }
    }

    /// `speaker` says `text`. Returns false if rate-limited (the line is still logged).
    pub fn say(&mut self, speaker: u64, name: &str, text: &str, priority: Priority) -> bool {
        let text = text.trim();
        if text.is_empty() {
            return false;
        }
        self.log.push_back(ChatLine { speaker, name: name.to_string(), text: text.to_string(), time: self.time });
        while self.log.len() > self.config.log_size {
            self.log.pop_front();
        }
        let now = self.time;
        let cfg = &self.config;
        if let Some(b) = self.bubbles.get_mut(&speaker) {
            if b.text == text && b.age < b.life {
                b.repeats += 1;
                b.age = 0.0;
                b.last_said = now;
                return true;
            }
            if now - b.last_said < cfg.min_interval as f64 && priority != Priority::Me {
                return false;
            }
        }
        let lines = wrap(text, cfg.line_chars, cfg.max_lines);
        let life = cfg.base_time + cfg.per_char * text.chars().count() as f32;
        self.bubbles.insert(
            speaker,
            Bubble { text: text.to_string(), lines, age: 0.0, life, repeats: 1, priority, last_said: now },
        );
        true
    }

    /// `speaker` plays an emote (an icon image).
    pub fn emote(&mut self, speaker: u64, image: ImageId) {
        // One emote per speaker at a time; replaying restarts it.
        self.emotes.retain(|e| e.speaker != speaker);
        self.emotes.push(Emote { speaker, image, age: 0.0 });
    }

    /// Forget a speaker (left the game).
    pub fn remove(&mut self, speaker: u64) {
        self.bubbles.remove(&speaker);
        self.emotes.retain(|e| e.speaker != speaker);
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt as f64;
        for b in self.bubbles.values_mut() {
            b.age += dt;
        }
        self.bubbles.retain(|_, b| b.age < b.life);
        for e in &mut self.emotes {
            e.age += dt;
        }
        self.emotes.retain(|e| e.age < EMOTE_LIFE);
    }

    pub fn log(&self) -> impl DoubleEndedIterator<Item = &ChatLine> {
        self.log.iter()
    }

    pub fn is_talking(&self, speaker: u64) -> bool {
        self.bubbles.contains_key(&speaker)
    }

    pub fn active_bubbles(&self) -> usize {
        self.bubbles.len()
    }

    /// Draw bubbles and emotes. `anchor(speaker)` gives the world point above a speaker's head
    /// (or `None` if it should not be shown, e.g. hidden by fog). `focus` is the screen point
    /// that matters most (usually the local player or the screen centre).
    pub fn draw(&mut self, w: &mut WorldDraw, focus: Vec2, anchor: impl Fn(u64) -> Option<Vec3>) {
        let s = w.ui_scale();
        let view = w.camera.viewport;
        let zoom_ok = w.camera.zoom >= self.config.min_zoom_for_text;
        let glyph = w.glyph_size(8.0);
        let pad = (4.0 * s).round();

        // Rank speakers that are on screen.
        struct Cand {
            id: u64,
            head: Vec2,
            score: f32,
        }
        let mut cands: Vec<Cand> = Vec::new();
        for (&id, b) in &self.bubbles {
            let Some(p) = anchor(id) else { continue };
            let head = w.to_screen(p);
            if !view.inset(-40.0 * s).contains(head) {
                continue;
            }
            let dist = (head - focus).length() / (view.w.max(view.h));
            let pri = match b.priority {
                Priority::Me => 1000.0,
                Priority::Friend => 10.0,
                Priority::Normal => 0.0,
            };
            cands.push(Cand { id, head, score: pri - b.age * 0.5 - dist * 4.0 });
        }
        cands.sort_by(|a, b| b.score.total_cmp(&a.score));
        let budget = if zoom_ok { self.config.max_bubbles } else { 0 };
        let (shown, hidden) = cands.split_at(budget.min(cands.len()));
        self.hidden_last_frame = hidden.len();

        // Markers for speakers over budget.
        for c in hidden {
            let r = Rect::new(c.head.x - 7.0 * s, c.head.y - 12.0 * s, 14.0 * s, 9.0 * s);
            w.ui_rect(r.min(), r.max(), Color::hex(0xf4efe2).with_alpha(0.85));
            for k in 0..3 {
                let x = r.x + (3.0 + k as f32 * 3.5) * s;
                w.ui_rect(Vec2::new(x, r.y + 4.0 * s), Vec2::new(x + 2.0 * s, r.y + 6.0 * s), Color::hex(0x2a2432));
            }
        }

        // Lay out shown bubbles: highest priority first, each pushed up until it overlaps none.
        let mut placed: Vec<Rect> = Vec::new();
        let mut bubble_of: HashMap<u64, Rect> = HashMap::new();
        for c in shown {
            let b = &self.bubbles[&c.id];
            let mut lines = b.lines.clone();
            if b.repeats > 1 {
                lines.last_mut().unwrap().push_str(&format!(" x{}", b.repeats));
            }
            let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32 * glyph + pad * 2.0;
            let height = lines.len() as f32 * glyph * 1.25 + pad * 2.0 - glyph * 0.25;
            let mut r = Rect::new(c.head.x - width * 0.5, c.head.y - height - 10.0 * s, width, height);
            r.x = r.x.clamp(view.x + 4.0, view.x + view.w - width - 4.0);
            for _ in 0..24 {
                match placed.iter().find(|o| overlaps(o, &r, 3.0 * s)) {
                    Some(o) => r.y = o.y - r.h - 4.0 * s,
                    None => break,
                }
            }
            placed.push(r);
            bubble_of.insert(c.id, r);
            let fade = ((b.life - b.age) / 0.4).clamp(0.0, 1.0) * (b.age / 0.12).clamp(0.0, 1.0);
            let (bg, fg, edge) = match b.priority {
                Priority::Me => (Color::hex(0xfff6d6), Color::hex(0x2a1e10), Color::hex(0xc8a45c)),
                Priority::Friend => (Color::hex(0xe4f4ff), Color::hex(0x10202a), Color::hex(0x5a9ad8)),
                Priority::Normal => (Color::hex(0xf4efe2), Color::hex(0x2a2432), Color::hex(0x6b5a3e)),
            };
            // Tail from the bubble to the head (long when the bubble was pushed away).
            let tail_top = Vec2::new(c.head.x.clamp(r.x + pad, r.x + r.w - pad), r.y + r.h);
            let steps = (((c.head.y - 6.0 * s) - tail_top.y) / (2.0 * s)).clamp(1.0, 60.0) as usize;
            for k in 0..steps {
                let t = k as f32 / steps as f32;
                let p = tail_top.lerp(c.head - Vec2::new(0.0, 6.0 * s), t);
                let half = (3.0 * (1.0 - t)).max(1.0) * s;
                w.ui_rect(p - Vec2::new(half, 0.0), p + Vec2::new(half, 2.0 * s), bg.with_alpha(fade));
            }
            w.ui_rect(r.min() - Vec2::splat(s), r.max() + Vec2::splat(s), edge.with_alpha(fade));
            w.ui_rect(r.min(), r.max(), bg.with_alpha(fade));
            for (i, l) in lines.iter().enumerate() {
                w.ui_text(Vec2::new(r.x + pad, r.y + pad + i as f32 * glyph * 1.25), l, glyph, fg.with_alpha(fade));
            }
        }

        // Emotes, merged by image and screen proximity.
        struct Group {
            image: ImageId,
            pos: Vec2,
            count: u32,
            age: f32,
        }
        let mut groups: Vec<Group> = Vec::new();
        let merge = self.config.emote_merge_px * s;
        for e in &self.emotes {
            let Some(p) = anchor(e.speaker) else { continue };
            // Float above the speaker's bubble if it has one.
            let head = match bubble_of.get(&e.speaker) {
                Some(r) => Vec2::new(r.x + r.w * 0.5, r.y + 2.0 * s),
                None => w.to_screen(p),
            };
            if !view.inset(-40.0 * s).contains(head) {
                continue;
            }
            match groups.iter_mut().find(|g| g.image == e.image && (g.pos - head).length() < merge) {
                Some(g) => {
                    g.pos = (g.pos * g.count as f32 + head) / (g.count + 1) as f32;
                    g.count += 1;
                    g.age = g.age.min(e.age);
                }
                None => groups.push(Group { image: e.image, pos: head, count: 1, age: e.age }),
            }
        }
        for g in groups {
            let t = g.age / EMOTE_LIFE;
            // Pop in with a little overshoot, float up, fade out.
            let pop = if g.age < 0.2 { 1.0 + 0.35 * (g.age / 0.2 * std::f32::consts::PI).sin() } else { 1.0 };
            let size = (18.0 * s * pop).round();
            let c = g.pos - Vec2::new(0.0, 22.0 * s + t * 18.0 * s);
            let alpha = (1.0 - (t - 0.75).max(0.0) / 0.25).clamp(0.0, 1.0);
            let r = size * 0.5 + 3.0 * s;
            w.ui_rect(c - Vec2::splat(r), c + Vec2::splat(r), Color::hex(0x2a2432).with_alpha(0.75 * alpha));
            w.ui_image(
                c - Vec2::splat(size * 0.5),
                c + Vec2::splat(size * 0.5),
                g.image,
                Color::WHITE.with_alpha(alpha),
            );
            if g.count > 1 {
                let text = format!("x{}", g.count);
                w.ui_text(c + Vec2::new(r + 2.0 * s, -glyph * 0.5), &text, glyph, Color::WHITE.with_alpha(alpha));
            }
        }
    }
}

fn overlaps(a: &Rect, b: &Rect, margin: f32) -> bool {
    a.x < b.x + b.w + margin && b.x < a.x + a.w + margin && a.y < b.y + b.h + margin && b.y < a.y + a.h + margin
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_and_truncates() {
        let l = wrap("the quick brown fox jumps over the lazy dog again and again", 12, 2);
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|x| x.chars().count() <= 12));
        assert!(l[1].ends_with("..."));
        assert_eq!(wrap("supercalifragilistic", 8, 3).len(), 3);
    }

    #[test]
    fn one_bubble_per_speaker_repeats_and_rate_limit() {
        let mut s = Speech::new(SpeechConfig::default());
        assert!(s.say(1, "a", "hello", Priority::Normal));
        assert!(s.say(1, "a", "hello", Priority::Normal));
        assert_eq!(s.bubbles[&1].repeats, 2);
        // A different line too soon is dropped from bubbles but logged.
        assert!(!s.say(1, "a", "spam", Priority::Normal));
        assert_eq!(s.log().count(), 3);
        s.update(0.5);
        assert!(s.say(1, "a", "later", Priority::Normal));
        assert_eq!(s.active_bubbles(), 1);
        s.update(60.0);
        assert_eq!(s.active_bubbles(), 0);
    }
}
