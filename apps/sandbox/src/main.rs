//! Knight Engine sandbox: one app, many demos. Run natively with `cargo run -p knight-sandbox`
//! or on the web with `scripts/web.sh serve`.

mod art;
mod common;
mod demos;
mod realtime;
mod tactics;

use knight_engine::*;

fn main() {
    let config = AppConfig {
        title: "Knight Engine Sandbox".into(),
        size: (1440, 900),
        canvas_id: Some("knight".into()),
        render: RenderSettings { mode: RenderMode::HiRes { msaa: true }, clear: Color::hex(0x0e0f16) },
    };
    run(config, |ctx| {
        let art = art::Art::load(ctx);
        log::info!("atlas {:.2} pages used", ctx.assets.atlas().usage());
        Box::new(demos::menu::Menu::new(art))
    });
}
