// Knight Engine world shader: terrain, sprites and overlays.

struct Globals {
    view_proj: mat4x4<f32>,
    // Light colour for terrain and sprites (day/night).
    ambient: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var atlas: texture_2d_array<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

// The atlas page is the whole part of U (see `Assets`).
fn atlas_sample(uv: vec2<f32>) -> vec4<f32> {
    let page = floor(uv.x);
    return textureSample(atlas, atlas_sampler, vec2<f32>(uv.x - page, uv.y), i32(page));
}

struct WorldIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) depth: f32,
};

struct Varyings {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_world(v: WorldIn) -> Varyings {
    var out: Varyings;
    var c = globals.view_proj * vec4<f32>(v.pos, 1.0);
    // Billboards carry the depth of their feet so they sort like painter's algorithm.
    if (v.depth >= 0.0) {
        c.z = v.depth * c.w;
    }
    out.clip = c;
    out.uv = v.uv;
    out.color = v.color;
    return out;
}

@fragment
fn fs_opaque(i: Varyings) -> @location(0) vec4<f32> {
    let c = atlas_sample(i.uv) * i.color;
    if (c.a < 0.5) {
        discard;
    }
    return vec4<f32>(c.rgb * globals.ambient.rgb, 1.0);
}

@fragment
fn fs_blend(i: Varyings) -> @location(0) vec4<f32> {
    let c = atlas_sample(i.uv) * i.color;
    return vec4<f32>(c.rgb * globals.ambient.rgb, c.a);
}

// Unlit: backgrounds and the on-top layer (markers, glows) ignore the ambient light.
@fragment
fn fs_unlit(i: Varyings) -> @location(0) vec4<f32> {
    return atlas_sample(i.uv) * i.color;
}

