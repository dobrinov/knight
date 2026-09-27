//! Terrain scaling benchmark: `cargo run -p knight-core --example bench_terrain --release`.
//! Builds square maps of growing size and measures load, per-frame cost and memory for a
//! normal view, a fly-over across the whole map and a zoomed-out view.

use std::time::{Duration, Instant};

use knight_core::glam::Vec3;
use knight_core::hex::{Layout, Offset, Parity, ValueNoise};
use knight_core::{Assets, Camera, Color, HexWorld, Material, Rect, Tile};

fn mb(verts: usize) -> f64 {
    verts as f64 * std::mem::size_of::<knight_core::mesh::Vertex>() as f64 / 1_048_576.0
}

fn main() {
    let assets = Assets::new();
    println!(
        "map          hexes     | load     | fill view        | frame   | fly-over peak RAM | zoomed out: chunks, fill, RAM"
    );
    for n in [128, 256, 512, 1024, 2048] {
        let t0 = Instant::now();
        let mut w = HexWorld::new(Layout::pointy(1.0));
        w.add_material(Material::color("g", Color::GREEN, Color::GRAY));
        let noise = ValueNoise::new(1);
        for row in 0..n {
            for col in 0..n {
                let h = Offset::new(col, row).to_hex_r(Parity::Odd);
                let e = noise.fbm(col as f32 * 0.05, row as f32 * 0.05, 4);
                w.set_tile(h, Tile::new((e * 12.0) as i16 - 3, 1));
            }
        }
        let load = t0.elapsed();
        let centre = w.hex_to_world(Offset::new(n / 2, n / 2).to_hex_r(Parity::Odd));
        let mut cam = Camera::new(centre, 32.0);
        cam.viewport = Rect::new(0.0, 0.0, 1920.0, 1080.0);

        // Fill the first view (meshing is spread over frames by the budget).
        let t1 = Instant::now();
        let mut frames = 0;
        let want = w.visible_chunk_keys(&cam).len();
        while w.visible_chunks(&cam, &assets).len() < want && frames < 1000 {
            frames += 1;
        }
        let fill = t1.elapsed();

        // Steady state.
        let t2 = Instant::now();
        for _ in 0..100 {
            w.visible_chunks(&cam, &assets);
        }
        let frame = t2.elapsed() / 100;

        // Fly across the whole map diagonally.
        let mut peak = 0;
        let steps = 400;
        let (lo, hi) = w.bounds();
        let mut worst = Duration::ZERO;
        for i in 0..=steps {
            let k = i as f32 / steps as f32;
            cam.target = Vec3::new(lo.x + (hi.x - lo.x) * k, 0.0, lo.y + (hi.y - lo.y) * k);
            let t = Instant::now();
            w.visible_chunks(&cam, &assets);
            worst = worst.max(t.elapsed());
            peak = peak.max(w.mesh_stats().1);
        }

        // Zoomed right out over the centre.
        cam.target = centre;
        cam.zoom = 4.0;
        let zc = w.visible_chunk_keys(&cam).len();
        let t3 = Instant::now();
        let mut zf = 0;
        while w.visible_chunks(&cam, &assets).len() < zc && zf < 5000 {
            zf += 1;
        }
        let zfill = t3.elapsed();
        println!(
            "{n:>4}x{n:<4} {:>9} | {:>7.1?} | {:>7.1?} ({frames:>2} fr) | {:>6.1?} | {:>6.1} MB (worst frame {:>6.1?}) | {zc:>5}, {:>7.1?} ({zf} fr), {:>6.1} MB",
            w.len(),
            load,
            fill,
            frame,
            mb(peak),
            worst,
            zfill,
            mb(w.mesh_stats().1)
        );
    }
}
