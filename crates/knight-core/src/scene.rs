//! Scenes, the scene stack and the per-app context.
//!
//! A game is a stack of [`Scene`]s: an adventure map, with a town screen pushed on top, with a
//! battle pushed on top of that, with a pause menu overlay on top of that. Only the top scene
//! gets input and updates; scenes below keep their state and are drawn when the scenes above are
//! overlays.

use std::any::Any;

use glam::Vec2;

use crate::frame::{Frame, FrameData};
use crate::{Assets, Color, Input};

/// Timing for the current frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Time {
    /// Seconds since the previous frame (clamped to avoid huge steps after stalls).
    pub dt: f32,
    /// Seconds since start.
    pub elapsed: f64,
    pub frame: u64,
    /// Smoothed frames per second.
    pub fps: f32,
    /// Fixed simulation step used for [`Scene::fixed_update`].
    pub fixed_dt: f32,
}

/// How the world is rasterised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    /// Render the world at `1/scale` resolution and upscale with nearest filtering: chunky,
    /// crisp pixels at any window size.
    PixelArt { scale: u32 },
    /// Render at full resolution with linear filtering and optional 4× MSAA.
    HiRes { msaa: bool },
}

#[derive(Clone, Copy, Debug)]
pub struct RenderSettings {
    pub mode: RenderMode,
    pub clear: Color,
}

impl Default for RenderSettings {
    fn default() -> Self {
        RenderSettings { mode: RenderMode::HiRes { msaa: true }, clear: Color::hex(0x101018) }
    }
}

/// Renderer statistics from the previous frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub draw_calls: u32,
    pub triangles: u32,
    pub chunks_drawn: u32,
    pub chunks_cached: u32,
    pub cpu_ms: f32,
}

/// Everything a scene can reach: input, time, assets, screen, settings.
pub struct Context {
    pub input: Input,
    pub time: Time,
    pub assets: Assets,
    /// Window size in physical pixels.
    pub screen: Vec2,
    /// OS / browser device pixel ratio.
    pub scale_factor: f32,
    /// Extra UI scale on top of the device pixel ratio.
    pub ui_zoom: f32,
    pub render: RenderSettings,
    pub stats: Stats,
    /// Sound effects and music (played by the runtime's audio backend).
    pub audio: crate::audio::Audio,
    /// Free-form status line published to the host: on the web it is mirrored to the canvas's
    /// `data-status` attribute (handy for automated tests), natively it is logged on change.
    pub status: String,
    /// Graphics backend in use, filled in by the runtime (e.g. "BrowserWebGpu").
    pub backend: String,
    pub(crate) quit: bool,
}

impl Context {
    pub fn new() -> Self {
        Context {
            input: Input::new(),
            time: Time { fixed_dt: 1.0 / 60.0, ..Default::default() },
            assets: Assets::new(),
            screen: Vec2::new(1280.0, 720.0),
            scale_factor: 1.0,
            ui_zoom: 1.0,
            render: RenderSettings::default(),
            stats: Stats::default(),
            audio: crate::audio::Audio::default(),
            status: String::new(),
            backend: String::new(),
            quit: false,
        }
    }

    /// Physical pixels per logical UI pixel.
    pub fn ui_scale(&self) -> f32 {
        (self.scale_factor * self.ui_zoom).max(0.5)
    }

    /// Full-window rectangle in physical pixels (a camera's default viewport).
    pub fn screen_rect(&self) -> crate::Rect {
        crate::Rect::new(0.0, 0.0, self.screen.x, self.screen.y)
    }

    /// Pixel scale in pixel-art mode, 1 otherwise.
    pub fn pixel_scale(&self) -> u32 {
        match self.render.mode {
            RenderMode::PixelArt { scale } => scale.max(1),
            RenderMode::HiRes { .. } => 1,
        }
    }

    /// Ask the app to exit (native) at the end of the frame.
    pub fn quit(&mut self) {
        self.quit = true;
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// What the scene stack should do after an update.
pub enum Transition {
    None,
    /// Put a new scene on top (the current one is paused).
    Push(Box<dyn Scene>),
    /// Remove the top scene and resume the one below.
    Pop,
    /// Remove the top scene and hand a result to the one below (e.g. battle outcome).
    PopWith(Box<dyn Any>),
    /// Replace the top scene.
    Switch(Box<dyn Scene>),
    /// Clear the stack and start over with this scene.
    Reset(Box<dyn Scene>),
    Quit,
}

pub trait Scene: 'static {
    /// Called once per frame for the top scene. Handle input and game logic here.
    fn update(&mut self, ctx: &mut Context) -> Transition;

    /// Called at a fixed rate ([`Time::fixed_dt`]) for the top scene. Use for deterministic
    /// simulation (RTS, physics).
    fn fixed_update(&mut self, _ctx: &mut Context) {}

    /// Record drawing for this frame.
    fn draw(&mut self, frame: &mut Frame);

    /// Overlays let the scenes below them be drawn (dialogs, pause menus).
    fn is_overlay(&self) -> bool {
        false
    }

    /// Called when the scene becomes the top scene again. `result` comes from
    /// [`Transition::PopWith`].
    fn on_resume(&mut self, _ctx: &mut Context, _result: Option<Box<dyn Any>>) {}

    /// Called when the scene is first shown.
    fn on_enter(&mut self, _ctx: &mut Context) {}
}

const FADE_TIME: f32 = 0.18;

/// The scene stack with fade transitions.
pub struct SceneStack {
    scenes: Vec<Box<dyn Scene>>,
    pending: Option<Transition>,
    /// 0 = idle; fading out while a transition is pending, then fading in.
    fade: f32,
    fading_in: bool,
    entered: bool,
    accumulator: f32,
}

impl SceneStack {
    pub fn new(root: Box<dyn Scene>) -> Self {
        SceneStack { scenes: vec![root], pending: None, fade: 0.0, fading_in: false, entered: false, accumulator: 0.0 }
    }

    pub fn len(&self) -> usize {
        self.scenes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scenes.is_empty()
    }

    /// Update the top scene and run transitions. Returns false when the app should exit.
    pub fn update(&mut self, ctx: &mut Context) -> bool {
        if !self.entered {
            self.entered = true;
            if let Some(s) = self.scenes.last_mut() {
                s.on_enter(ctx);
            }
        }
        let dt = ctx.time.dt;
        if self.pending.is_some() {
            self.fade += dt / FADE_TIME;
            if self.fade >= 1.0 {
                self.fade = 1.0;
                let t = self.pending.take().unwrap();
                self.apply(t, ctx);
                self.fading_in = true;
            }
        } else if self.fading_in {
            self.fade -= dt / FADE_TIME;
            if self.fade <= 0.0 {
                self.fade = 0.0;
                self.fading_in = false;
            }
        }
        if self.pending.is_none() && !ctx.quit {
            let Some(top) = self.scenes.last_mut() else { return false };
            self.accumulator = (self.accumulator + dt).min(0.25);
            let step = ctx.time.fixed_dt;
            while self.accumulator >= step {
                top.fixed_update(ctx);
                self.accumulator -= step;
            }
            match top.update(ctx) {
                Transition::None => {}
                Transition::Quit => ctx.quit = true,
                // Popping an overlay is instant; everything else fades.
                Transition::Pop if top.is_overlay() => self.apply(Transition::Pop, ctx),
                Transition::PopWith(r) if top.is_overlay() => self.apply(Transition::PopWith(r), ctx),
                Transition::Push(s) if s.is_overlay() => self.apply(Transition::Push(s), ctx),
                t => self.pending = Some(t),
            }
        }
        !ctx.quit && !self.scenes.is_empty()
    }

    fn apply(&mut self, t: Transition, ctx: &mut Context) {
        match t {
            Transition::None | Transition::Quit => {}
            Transition::Push(mut s) => {
                s.on_enter(ctx);
                self.scenes.push(s);
            }
            Transition::Pop => {
                self.scenes.pop();
                if let Some(s) = self.scenes.last_mut() {
                    s.on_resume(ctx, None);
                }
            }
            Transition::PopWith(r) => {
                self.scenes.pop();
                if let Some(s) = self.scenes.last_mut() {
                    s.on_resume(ctx, Some(r));
                }
            }
            Transition::Switch(mut s) => {
                self.scenes.pop();
                s.on_enter(ctx);
                self.scenes.push(s);
            }
            Transition::Reset(mut s) => {
                self.scenes.clear();
                s.on_enter(ctx);
                self.scenes.push(s);
            }
        }
        if self.scenes.is_empty() {
            ctx.quit = true;
        }
    }

    /// Draw visible scenes bottom to top.
    pub fn draw(&mut self, ctx: &Context, data: &mut FrameData) {
        data.reset(ctx.render.clear);
        let mut first = self.scenes.len().saturating_sub(1);
        while first > 0 && self.scenes[first].is_overlay() {
            first -= 1;
        }
        let mut frame = Frame::new(ctx, data);
        for s in &mut self.scenes[first..] {
            s.draw(&mut frame);
        }
        if ctx.input.using_gamepad() && ctx.input.gamepad_cursor {
            frame.pad_cursor();
        }
        frame.fade(self.fade);
    }

    /// Carry UI hit regions from this frame's draw into next frame's input.
    pub fn feed_ui(ctx: &mut Context, data: &FrameData) {
        ctx.input.ui_rects.clone_from(&data.ui_rects);
        ctx.input.ui_clicked.clone_from(&data.ui_clicked);
        if !data.ui_clicked.is_empty()
            && let Some(click) = ctx.audio.ui_click
        {
            ctx.audio.play(click);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Probe {
        log: Rc<RefCell<Vec<String>>>,
        name: &'static str,
        next: Option<Transition>,
        overlay: bool,
    }

    impl Scene for Probe {
        fn update(&mut self, _: &mut Context) -> Transition {
            self.log.borrow_mut().push(format!("update {}", self.name));
            self.next.take().unwrap_or(Transition::None)
        }
        fn draw(&mut self, _: &mut Frame) {
            self.log.borrow_mut().push(format!("draw {}", self.name));
        }
        fn is_overlay(&self) -> bool {
            self.overlay
        }
        fn on_resume(&mut self, _: &mut Context, r: Option<Box<dyn Any>>) {
            let v = r.and_then(|r| r.downcast::<i32>().ok()).map(|b| *b);
            self.log.borrow_mut().push(format!("resume {} {:?}", self.name, v));
        }
    }

    #[test]
    fn push_pop_with_result_and_overlays() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let child =
            Probe { log: log.clone(), name: "battle", next: Some(Transition::PopWith(Box::new(7))), overlay: false };
        let root =
            Probe { log: log.clone(), name: "map", next: Some(Transition::Push(Box::new(child))), overlay: false };
        let mut stack = SceneStack::new(Box::new(root));
        let mut ctx = Context::new();
        ctx.time.dt = 0.1;
        let mut data = FrameData::default();
        let mut faded_frames = 0;
        for _ in 0..12 {
            stack.update(&mut ctx);
            stack.draw(&ctx, &mut data);
            // The transition fade is plain UI geometry (a full-screen quad) at the end.
            faded_frames += usize::from(!data.ui.is_empty());
        }
        assert!(faded_frames > 0 && faded_frames < 12, "faded during the transition only: {faded_frames}");
        let log = log.borrow();
        assert!(log.contains(&"update battle".to_string()));
        assert!(log.contains(&"resume map Some(7)".to_string()));
        assert_eq!(stack.len(), 1);

        let pause = Probe { log: Rc::new(RefCell::new(Vec::new())), name: "pause", next: None, overlay: true };
        let plog = pause.log.clone();
        let root =
            Probe { log: plog.clone(), name: "map", next: Some(Transition::Push(Box::new(pause))), overlay: false };
        let mut stack = SceneStack::new(Box::new(root));
        stack.update(&mut ctx);
        stack.draw(&ctx, &mut data);
        let l = plog.borrow();
        // The overlay is pushed instantly and both scenes draw.
        assert_eq!(&l[l.len() - 2..], &["draw map".to_string(), "draw pause".to_string()]);
    }
}
