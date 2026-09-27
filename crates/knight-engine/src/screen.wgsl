// Knight Engine screen shaders: full-screen blit of the world target, and UI.

// The blit reads the scene target; the UI reads the atlas pages (a texture array). Each entry
// point uses only one of the two, so they can share the binding slot.
@group(1) @binding(0) var scene: texture_2d<f32>;
@group(1) @binding(0) var atlas: texture_2d_array<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

// The atlas page is the whole part of U (see `Assets`).
fn atlas_sample(uv: vec2<f32>) -> vec4<f32> {
    let page = floor(uv.x);
    return textureSample(atlas, atlas_sampler, vec2<f32>(uv.x - page, uv.y), i32(page));
}

struct Varyings {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct Screen {
    // Physical window size.
    size: vec2<f32>,
    // Blit: scene target size × pixel scale (so each target texel covers `scale` pixels).
    blit_size: vec2<f32>,
    // 1.0 when the surface format is sRGB and colours must be linearised on output.
    linear_out: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> screen: Screen;

fn encode(c: vec3<f32>) -> vec3<f32> {
    if (screen.linear_out > 0.5) {
        return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
    }
    return c;
}

struct BlitOut {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn vs_blit(@builtin(vertex_index) i: u32) -> BlitOut {
    let x = f32((i << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(i & 2u) * 2.0 - 1.0;
    var out: BlitOut;
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

@fragment
fn fs_blit(i: BlitOut) -> @location(0) vec4<f32> {
    let uv = i.clip.xy / screen.blit_size;
    let c = textureSample(scene, atlas_sampler, uv);
    return vec4<f32>(encode(c.rgb), 1.0);
}

struct UiIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_ui(v: UiIn) -> Varyings {
    var out: Varyings;
    let ndc = vec2<f32>(v.pos.x / screen.size.x * 2.0 - 1.0, 1.0 - v.pos.y / screen.size.y * 2.0);
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = v.uv;
    out.color = v.color;
    return out;
}

@fragment
fn fs_ui(i: Varyings) -> @location(0) vec4<f32> {
    let c = atlas_sample(i.uv) * i.color;
    return vec4<f32>(encode(c.rgb), c.a);
}
