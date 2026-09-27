//! The app loop: window, events, scene stack and rendering, for native and web.

use std::sync::Arc;

use knight_core::glam::Vec2;
use knight_core::{Context, FrameData, Key, MouseButton, RenderSettings, Scene, SceneStack};
use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::audio_out::AudioOut;
use crate::gamepad_in::GamepadIn;
use crate::renderer::Renderer;

/// App configuration.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub title: String,
    /// Initial window size in logical pixels (native only).
    pub size: (u32, u32),
    /// Web: id of an existing `<canvas>` to render into. If missing, a canvas is appended to
    /// `<body>`.
    pub canvas_id: Option<String>,
    pub render: RenderSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            title: "Knight Engine".into(),
            size: (1280, 800),
            canvas_id: Some("knight".into()),
            render: RenderSettings::default(),
        }
    }
}

type Init = Box<dyn FnOnce(&mut Context) -> Box<dyn Scene>>;

enum UserEvent {
    Gpu(Result<Renderer, String>),
}

struct App {
    config: AppConfig,
    proxy: Option<EventLoopProxy<UserEvent>>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    ctx: Context,
    init: Option<Init>,
    stack: Option<SceneStack>,
    frame: FrameData,
    last: Instant,
    start: Instant,
    published_status: String,
    audio: Option<AudioOut>,
    gamepads: GamepadIn,
}

/// Run an app. `init` builds the first scene once the GPU is ready (load assets there).
pub fn run(config: AppConfig, init: impl FnOnce(&mut Context) -> Box<dyn Scene> + 'static) {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        let _ = console_log::init_with_level(log::Level::Info);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"),
        )
        .try_init();
    }

    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let mut ctx = Context::new();
    ctx.render = config.render;
    let app = App {
        config,
        proxy: Some(event_loop.create_proxy()),
        window: None,
        renderer: None,
        ctx,
        init: Some(Box::new(init)),
        stack: None,
        frame: FrameData::default(),
        last: Instant::now(),
        start: Instant::now(),
        published_status: String::new(),
        audio: AudioOut::new(),
        gamepads: GamepadIn::new(),
    };
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = app;
        event_loop.run_app(&mut app).expect("event loop failed");
    }
}

async fn create_renderer(window: Arc<Window>) -> Result<Renderer, String> {
    let size = window.inner_size();
    let desc = wgpu::InstanceDescriptor::new_without_display_handle();
    #[cfg(target_arch = "wasm32")]
    let instance = wgpu::util::new_instance_with_webgpu_detection(desc).await;
    #[cfg(not(target_arch = "wasm32"))]
    let instance = wgpu::Instance::new(desc);
    let surface = instance.create_surface(window).map_err(|e| format!("create_surface: {e}"))?;
    Renderer::new(&instance, surface, size.width, size.height).await
}

impl App {
    /// One frame: update, draw, render. Returns false once the game asked to quit.
    fn tick(&mut self) -> bool {
        let (Some(renderer), Some(window)) = (self.renderer.as_mut(), self.window.as_ref()) else { return true };
        #[cfg(target_arch = "wasm32")]
        sync_canvas_size(window, renderer);
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let t = &mut self.ctx.time;
        t.dt = dt;
        t.elapsed = (now - self.start).as_secs_f64();
        t.frame += 1;
        t.fps = if t.fps == 0.0 { 60.0 } else { t.fps * 0.95 + (1.0 / dt.max(1e-4)) * 0.05 };
        let (w, h) = renderer.size();
        self.ctx.screen = Vec2::new(w as f32, h as f32);
        self.ctx.scale_factor = window.scale_factor() as f32;

        if self.stack.is_none() {
            let init = self.init.take().expect("init");
            self.stack = Some(SceneStack::new(init(&mut self.ctx)));
        }
        self.gamepads.poll(&mut self.ctx.input);
        let (screen, scale) = (self.ctx.screen, self.ctx.scale_factor);
        self.ctx.input.apply_gamepad(dt, screen, scale);
        let stack = self.stack.as_mut().unwrap();
        let cpu = Instant::now();
        let alive = stack.update(&mut self.ctx);
        let elapsed = self.ctx.time.elapsed;
        self.ctx.assets.update(elapsed);
        stack.draw(&self.ctx, &mut self.frame);
        SceneStack::feed_ui(&mut self.ctx, &self.frame);
        let cpu_ms = cpu.elapsed().as_secs_f32() * 1000.0;
        if let Some(a) = &mut self.audio {
            a.submit(&mut self.ctx.audio);
        } else {
            self.ctx.audio.drain();
        }
        let mut stats = renderer.render(&self.frame, &self.ctx.assets, self.ctx.render.mode);
        stats.cpu_ms = cpu_ms;
        self.ctx.stats = stats;
        self.ctx.input.end_frame();
        if self.ctx.status != self.published_status {
            self.published_status.clone_from(&self.ctx.status);
            publish_status(window, &self.published_status);
        }
        alive
    }
}

/// Keep the canvas backing store at CSS size × devicePixelRatio (crisp on high-DPI screens).
#[cfg(target_arch = "wasm32")]
fn sync_canvas_size(window: &Window, renderer: &mut Renderer) {
    use winit::platform::web::WindowExtWebSys;
    let (Some(canvas), Some(win)) = (window.canvas(), web_sys::window()) else { return };
    let dpr = win.device_pixel_ratio();
    let w = ((canvas.client_width() as f64 * dpr).round() as u32).max(1);
    let h = ((canvas.client_height() as f64 * dpr).round() as u32).max(1);
    if (w, h) != renderer.size() {
        canvas.set_width(w);
        canvas.set_height(h);
        renderer.resize(w, h);
    }
}

/// Host name of the page the game was loaded from (web only), e.g. to find the relay server.
pub fn web_host() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window().and_then(|w| w.location().hostname().ok()).filter(|h| !h.is_empty())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

#[cfg(target_arch = "wasm32")]
fn publish_status(window: &Window, status: &str) {
    use winit::platform::web::WindowExtWebSys;
    if let Some(canvas) = window.canvas() {
        let _ = canvas.set_attribute("data-status", status);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn publish_status(_window: &Window, status: &str) {
    log::debug!("status: {status}");
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes().with_title(self.config.title.clone());
        // On the web the page's CSS sizes the canvas; natively we pick the window size.
        #[cfg(not(target_arch = "wasm32"))]
        {
            attrs = attrs.with_inner_size(winit::dpi::LogicalSize::new(self.config.size.0, self.config.size.1));
        }
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            let document = web_sys::window().and_then(|w| w.document()).expect("document");
            let canvas = self
                .config
                .canvas_id
                .as_ref()
                .and_then(|id| document.get_element_by_id(id))
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
            attrs = match canvas {
                Some(c) => attrs.with_canvas(Some(c)),
                None => attrs.with_append(true),
            }
            .with_prevent_default(true)
            .with_focusable(true);
        }
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        self.window = Some(window.clone());
        let proxy = self.proxy.take().expect("proxy");
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let _ = proxy.send_event(UserEvent::Gpu(create_renderer(window).await));
        });
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = proxy.send_event(UserEvent::Gpu(pollster::block_on(create_renderer(window))));
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Gpu(Ok(mut r)) => {
                if let Some(w) = &self.window {
                    let s = w.inner_size();
                    r.resize(s.width, s.height);
                    w.request_redraw();
                }
                self.ctx.backend = r.backend.clone();
                self.renderer = Some(r);
            }
            UserEvent::Gpu(Err(e)) => {
                log::error!("knight: GPU initialisation failed: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Browsers only start audio from inside a user gesture.
        if matches!(event, WindowEvent::MouseInput { .. } | WindowEvent::KeyboardInput { .. } | WindowEvent::Touch(_))
            && let Some(a) = &mut self.audio
        {
            a.unlock();
        }
        let input = &mut self.ctx.input;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                // A web page has nowhere to "exit" to, so quitting only ends native apps.
                if !self.tick() && cfg!(not(target_arch = "wasm32")) {
                    event_loop.exit();
                    return;
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                input.on_mouse_move(Vec2::new(position.x as f32, position.y as f32));
            }
            WindowEvent::CursorEntered { .. } => input.hovering = true,
            WindowEvent::CursorLeft { .. } => input.hovering = false,
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                input.on_button(b, state == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 60.0,
                };
                input.on_wheel(lines);
            }
            WindowEvent::PinchGesture { delta, .. } => input.on_pinch(delta as f32),
            WindowEvent::Touch(t) => {
                let p = Vec2::new(t.location.x as f32, t.location.y as f32);
                match t.phase {
                    TouchPhase::Started => input.on_touch_start(t.id, p),
                    TouchPhase::Moved => input.on_touch_move(t.id, p),
                    TouchPhase::Ended | TouchPhase::Cancelled => input.on_touch_end(t.id),
                }
            }
            WindowEvent::Focused(focused) => input.on_focus(focused),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key
                    && let Some(k) = map_key(code)
                {
                    input.on_key(k, event.state == ElementState::Pressed);
                }
                if event.state == ElementState::Pressed
                    && let Some(text) = &event.text
                {
                    input.on_text(text);
                }
            }
            _ => {}
        }
    }
}

fn map_key(k: KeyCode) -> Option<Key> {
    use KeyCode as C;
    Some(match k {
        C::KeyA => Key::A,
        C::KeyB => Key::B,
        C::KeyC => Key::C,
        C::KeyD => Key::D,
        C::KeyE => Key::E,
        C::KeyF => Key::F,
        C::KeyG => Key::G,
        C::KeyH => Key::H,
        C::KeyI => Key::I,
        C::KeyJ => Key::J,
        C::KeyK => Key::K,
        C::KeyL => Key::L,
        C::KeyM => Key::M,
        C::KeyN => Key::N,
        C::KeyO => Key::O,
        C::KeyP => Key::P,
        C::KeyQ => Key::Q,
        C::KeyR => Key::R,
        C::KeyS => Key::S,
        C::KeyT => Key::T,
        C::KeyU => Key::U,
        C::KeyV => Key::V,
        C::KeyW => Key::W,
        C::KeyX => Key::X,
        C::KeyY => Key::Y,
        C::KeyZ => Key::Z,
        C::Digit0 => Key::Digit0,
        C::Digit1 => Key::Digit1,
        C::Digit2 => Key::Digit2,
        C::Digit3 => Key::Digit3,
        C::Digit4 => Key::Digit4,
        C::Digit5 => Key::Digit5,
        C::Digit6 => Key::Digit6,
        C::Digit7 => Key::Digit7,
        C::Digit8 => Key::Digit8,
        C::Digit9 => Key::Digit9,
        C::Space => Key::Space,
        C::Enter | C::NumpadEnter => Key::Enter,
        C::Escape => Key::Escape,
        C::Tab => Key::Tab,
        C::Backspace => Key::Backspace,
        C::Delete => Key::Delete,
        C::ArrowLeft => Key::Left,
        C::ArrowRight => Key::Right,
        C::ArrowUp => Key::Up,
        C::ArrowDown => Key::Down,
        C::PageUp => Key::PageUp,
        C::PageDown => Key::PageDown,
        C::Home => Key::Home,
        C::End => Key::End,
        C::ShiftLeft | C::ShiftRight => Key::Shift,
        C::ControlLeft | C::ControlRight => Key::Control,
        C::AltLeft | C::AltRight => Key::Alt,
        C::SuperLeft | C::SuperRight => Key::Super,
        C::F1 => Key::F1,
        C::F2 => Key::F2,
        C::F3 => Key::F3,
        C::F4 => Key::F4,
        C::F5 => Key::F5,
        C::F6 => Key::F6,
        C::F7 => Key::F7,
        C::F8 => Key::F8,
        C::F9 => Key::F9,
        C::F10 => Key::F10,
        C::F11 => Key::F11,
        C::F12 => Key::F12,
        C::Minus | C::NumpadSubtract => Key::Minus,
        C::Equal | C::NumpadAdd => Key::Equal,
        C::Comma => Key::Comma,
        C::Period => Key::Period,
        C::Slash => Key::Slash,
        C::BracketLeft => Key::BracketLeft,
        C::BracketRight => Key::BracketRight,
        C::Backquote => Key::Backquote,
        _ => return None,
    })
}
