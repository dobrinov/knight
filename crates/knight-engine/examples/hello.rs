//! Minimal Knight Engine app (same as the README quick start): `cargo run -p knight-engine --example hello`.

use knight_engine::*;

struct Hello {
    world: HexWorld,
    camera: Camera,
    controls: CameraController,
}

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
