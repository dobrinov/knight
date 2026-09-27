# Knight Engine

A hex-based, isometric 2.5D game engine in Rust for strategy and RPG games: Heroes-style
adventure maps, Diablo-style RPGs, Warcraft-style squad strategy, Railroad-Tycoon-style builders.
Built on **wgpu** and **winit**, so one codebase runs on the web (WebGPU, falling back to WebGL2),
Windows, macOS, Linux, iOS and Android. Hex math follows
[Red Blob Games' hexagon guide](https://www.redblobgames.com/grids/hexagons/).

## Features

- **Hex grids:** axial/cube/offset/doubled coordinates, pointy and flat layouts, rings, spirals,
  lines, rotation and reflection, and map shapes (hexagon, rectangle, rhombus, triangle).
- **Terrain:** per-hex height levels (hills and depressions), a water plane with visible
  submerged ground, textured materials, stacked cliff walls and a bevel shading grid.
- **Isometric camera:** pan (drag, keys, or pushing the pointer against the screen edge), zoom at the cursor, and a configurable viewing angle (from 30° to
  top-down). The camera orientation is fixed (no rotation). Controls cover mouse drag, wheel,
  keyboard, screen-edge scrolling, pinch and two-finger pan.
- **Rendering:** chunked terrain meshes, built only for the chunks in view. Meshing is spread
  over frames, and chunks that go unused are evicted, so a million-hex map loads in a fraction
  of a second and uses about as much memory as a small one. Sprites are depth-correct and upright,
  with shadows. Overlays (fills, outlines, paths, rings, decals) can be drawn on top of
  everything. Also: world-anchored labels and bars, several cameras per frame, gradient
  backdrops, and a `Minimap` image (one pixel per hex, any map size, re-rendered only on change).
- **Asset pack (`knight-assets`):** bundled pixel art, loaded on demand.
  - 19 terrains (seamless tops and cliffs; water and lava animate) and 10 prop sheets (trees,
    rocks, bushes, mountains), plus fire, smoke and burn decals.
  - 45 characters: humans, orcs and elves units and heroes, and monsters. Each has idle, walk,
    attack, hurt and death, plus a second attack, shot, cast or work animation.
  - 38 buildings, each with an idle animation plus construction, damaged and ruined states.
  - Everything is recoloured to any team colour.
  - The atlas grows extra 2048² pages as needed. Animated images update in place.
- **Pixel art and hi-res:** a pixel-art mode (low-res target, nearest upscale, pixel snapping,
  pixel-perfect zoom levels) or a hi-res mode (linear filtering, 4× MSAA). The UI stays crisp on
  high-DPI screens.
- **Picking:** elevation-aware. Rays hit tile tops and walls, so tall hexes correctly hide the
  ones behind them.
- **Hex navigation:** `HexMover` is the only way to move. It walks routes of adjacent hexes, never
  stops between hexes, and walks at the speed the terrain allows (roads fast, forests and snow
  slow, climbing slower). `Reservations` let real-time units queue instead of overlapping. The
  same terrain costs become movement points in turn-based games.
- **Closing-in hazards:** `ShrinkingZone` is a phased safe circle that closes toward random
  points (the next circle is known in advance) with damage outside it. `Wildfire` is fire that
  spreads by terrain flammability and burns out to ash. Together they give battle royale, "the
  map shrinks" or storm modes, in turn-based (ticks = turns) or real-time (ticks = seconds) games.
- **Hybrid turns:** `DayClock` and `ActionBudget` give timed days instead of "End turn": everyone
  acts at the same time in real time, each with a daily budget of movement points and actions,
  and the day ends when its timer runs out or everyone is ready (like simultaneous turns).
- **Multiplayer chat:** chat bubbles and emotes that stay readable in crowds (a budget of visible
  bubbles ranked by priority, "..." markers for the rest, no overlaps, repeats collapsed, and
  identical emotes merged with a count), plus a chat log and a text field widget.
- **Light and FX:** per-camera ambient light for day/night and weather, particles (sparks,
  smoke, fire), territory borders and hex-edge lines for rivers and walls.
- **Gameplay helpers:** A* with arbitrary step costs, movement ranges within a budget, flow
  fields for crowds, height-aware line of sight and field of view, fog of war
  (hidden/explored/visible), a seeded RNG and value noise.
- **Scenes:** a scene stack for multiple UI levels (map → town → battle) with fade transitions
  and results passed back down (`PopWith`).
- **Save and load:** a world's map (layout, heights, materials by name, water, fog of war) round
  trips through `HexWorld::to_bytes` / `load_bytes`, a compact self-contained blob for files,
  browser storage or network messages.
- **UI:** immediate-mode panels, buttons, bars, tooltips, wrapped text and images, with a
  built-in pixel font.
- **Input:** mouse, keyboard, touch (tap, drag, pinch), click-vs-drag detection and UI hit
  testing, plus gamepads (gilrs natively, the Gamepad API on the web). By default the first pad
  drives a virtual cursor, so every game is controller-playable: left stick = cursor,
  A = click, B = right-click, right stick = pan, triggers or bumpers = zoom, Start = Enter,
  Select = Escape.
- **Audio:** a mixer with positional sound (panned and attenuated by screen position), looping
  music and a UI click sound. Sounds come from WAV files or are synthesized at startup
  (sfxr-style effects and a chiptune generator), so games need no audio files. Output uses cpal
  natively and WebAudio in the browser, unlocked on the first click.
- **Networking (`knight-net`, the engine's default `net` feature):** a compact binary codec, a
  relay protocol with rooms, WebSocket transports for native and web, an in-process hub (tests,
  hot seat, bots), and deterministic lockstep for turn-based or fixed-tick games. `apps/relay` is
  a small WebSocket relay server. Offline games can turn the feature off.

## Workspace

```
crates/knight-hex      hex math, pathfinding, vision, noise (no dependencies)
crates/knight-core     worlds, camera, picking, meshing, frame/UI, scenes, input, assets (no GPU)
crates/knight-assets   the bundled pixel-art pack (terrain, props, fire, characters, buildings)
crates/knight-net      codec, relay protocol, WebSocket / in-process transports, lockstep
crates/knight-engine   wgpu renderer, winit runtime, audio out, gamepads; re-exports everything
apps/relay             WebSocket relay server for multiplayer (rooms)
apps/sandbox           demo app (native + web); not part of the engine, games depend on knight-engine only
scripts/web.sh         web build / serve
```

## Quick start

```rust
use knight_engine::*;

struct Hello { world: HexWorld, camera: Camera, controls: CameraController }

impl Scene for Hello {
    fn update(&mut self, ctx: &mut Context) -> Transition {
        self.camera.viewport = ctx.screen_rect();
        self.camera.dpr = ctx.scale_factor;
        self.controls.update(&mut self.camera, &ctx.input, ctx.time.dt, ctx.input.pointer_over_ui());
        Transition::None
    }

    fn draw(&mut self, f: &mut Frame) {
        let hover = self.world.pick(self.camera.screen_ray(f.ctx.input.mouse));
        let mut w = f.world(&mut self.world, &self.camera);
        if let Some(p) = hover {
            w.hex_outline(p.hex, Color::YELLOW, 0.1);
        }
    }
}

fn main() {
    run(AppConfig::default(), |_ctx| {
        let mut world = HexWorld::new(hex::Layout::pointy(1.0));
        for h in hex::shapes::hexagon(hex::Hex::ORIGIN, 10) {
            world.set_tile(h, Tile::new((h.q.abs() % 3) as i16, 0));
        }
        Box::new(Hello { world, camera: Camera::new(glam::Vec3::ZERO, 40.0), controls: CameraController::default() })
    });
}
```

Run it with `cargo run -p knight-engine --example hello`.

## Running the sandbox

You need Rust from rustup; the toolchain file adds the `wasm32-unknown-unknown` target. For the
web build you also need `wasm-bindgen-cli` 0.2.129.

```sh
cargo run -p knight-sandbox --release   # native window
scripts/web.sh serve                    # web: http://localhost:8080
cargo run -p knight-relay --release     # relay for the Online Play demo (ws://localhost:9001)
cargo test --workspace
cargo run -p knight-core --example bench_terrain --release   # large-map benchmark
```

The menu has 17 demos, each named after the features it shows:
- Terrain & Camera, Picking & Editing, Pathfinding, Vision & Fog, and Overlays, Light & FX.
- Scenes & UI Levels (map → town → battle).
- Turn-Based: 1 Unit and Turn-Based: Squads.
- Realtime: 1 Unit and Realtime: Squads.
- Hybrid: Timed Days, Battle Royale, Chat & Emotes, Online Play (open two tabs, or practice with a bot), Routes & Transport, and a Stress Test (32k to 840k hexes with a minimap, and 2,000 units pathing hex to hex).
- Asset Pack: every terrain, prop, character and building in the bundled pack, with animations,
  building states and team colours.

Press a demo's key to open it (shown on its card) and `Esc` to go back. `P` toggles pixel-art rendering, `M` music and `N` mute. With a gamepad, move the cursor with the left stick and press A.

## Platforms

| Target | Status |
| --- | --- |
| Web (WebGPU / WebGL2) | Built and verified in Chrome (both backends, DPR 1 and 2, phone viewport); `?backend=webgl2` forces the fallback |
| macOS (Metal) | Runs natively |
| Windows (DX12/Vulkan), Linux (Vulkan/GL) | Supported by wgpu + winit, not yet tested here |
| iOS, Android | Supported by wgpu + winit; needs packaging (e.g. `cargo-mobile2`), not set up yet |
| Steam | A native desktop build; Steamworks integration not included |
| PlayStation, Xbox | Need NDA platform SDKs and custom wgpu/winit backends; out of scope for this repo |

Not implemented yet: NAT-traversing peer-to-peer (WebRTC/Steam) transports and
server-authoritative rollback netcode. The relay plus lockstep cover turn-based, hybrid and
fixed-tick games.
