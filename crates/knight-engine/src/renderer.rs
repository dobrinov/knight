//! wgpu renderer: draws a [`FrameData`] to the window.
//!
//! Pipeline per frame:
//! 1. World passes render into an offscreen target (low resolution in pixel-art mode, full
//!    resolution with optional MSAA in hi-res mode), sharing one depth buffer.
//! 2. The target is blitted to the window (nearest filtering for pixel art).
//! 3. UI is drawn on top at native resolution.

use std::collections::HashMap;
use std::sync::Arc;

use knight_core::mesh::{UiVertex, Vertex, WorldMesh};
use knight_core::{Assets, FrameData, RenderMode, Stats};
use wgpu::util::DeviceExt;

const MAX_PASSES: usize = 32;
const UNIFORM_STRIDE: u64 = 256;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

impl GpuMesh {
    fn new(device: &wgpu::Device, mesh: &WorldMesh) -> Option<GpuMesh> {
        if mesh.is_empty() {
            return None;
        }
        Some(GpuMesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk vertices"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            count: mesh.indices.len() as u32,
        })
    }
}

struct GpuChunk {
    version: u64,
    // Keep the source Arc so an unchanged chunk is recognised cheaply.
    source: Arc<WorldMesh>,
    opaque: Option<GpuMesh>,
    transparent: Option<GpuMesh>,
    last_used: u64,
}

/// A growable GPU buffer rewritten every frame.
struct DynBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl DynBuffer {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        let capacity = 1 << 16;
        DynBuffer {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: capacity,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
            usage,
            label,
        }
    }

    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) {
        let size = (data.len() as u64).next_multiple_of(4);
        if size > self.capacity {
            self.capacity = size.next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: self.capacity,
                usage: self.usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !data.is_empty() {
            if data.len().is_multiple_of(4) {
                queue.write_buffer(&self.buffer, 0, data);
            } else {
                let mut padded = data.to_vec();
                padded.resize(size as usize, 0);
                queue.write_buffer(&self.buffer, 0, &padded);
            }
        }
    }
}

struct Targets {
    width: u32,
    height: u32,
    samples: u32,
    color: wgpu::TextureView,
    msaa: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    blit_bind: wgpu::BindGroup,
}

struct Pipelines {
    samples: u32,
    opaque: wgpu::RenderPipeline,
    blend: wgpu::RenderPipeline,
    fill: wgpu::RenderPipeline,
    on_top: wgpu::RenderPipeline,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    linear_out: bool,
    pub backend: String,

    /// The atlas as a texture array, one layer per atlas page.
    atlas: wgpu::Texture,
    atlas_layers: u32,
    atlas_version: u64,
    atlas_nearest: wgpu::BindGroup,
    atlas_linear: wgpu::BindGroup,
    atlas_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler_nearest: wgpu::Sampler,
    sampler_linear: wgpu::Sampler,

    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    screen_uniform: wgpu::Buffer,
    screen_bind: wgpu::BindGroup,

    world_shader: wgpu::ShaderModule,
    world_layout: wgpu::PipelineLayout,
    pipelines: Option<Pipelines>,
    blit_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    targets: Option<Targets>,

    chunks: HashMap<(u64, (i32, i32)), GpuChunk>,
    dyn_vertices: DynBuffer,
    dyn_indices: DynBuffer,
    ui_vertices: DynBuffer,
    ui_indices: DynBuffer,
    /// Scratch space reused every frame: concatenated per-pass geometry and the uniform block.
    scratch: WorldMesh,
    uniform_data: Vec<u8>,
    frame: u64,
}

fn world_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRS: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Unorm8x4, 3 => Float32];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRS,
    }
}

fn ui_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<UiVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRS,
    }
}

impl Renderer {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Renderer, String> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| format!("no suitable GPU adapter: {e}"))?;
        let info = adapter.get_info();
        let backend = format!("{:?} ({})", info.backend, info.name);
        log::info!("knight: using {backend}");
        let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("knight device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| format!("request_device failed: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let linear_out = format.is_srgb();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: Default::default(),
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let texture_layout_of = |view_dimension, label| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            })
        };
        let texture_layout = texture_layout_of(wgpu::TextureViewDimension::D2, "texture layout");
        let atlas_layout = texture_layout_of(wgpu::TextureViewDimension::D2Array, "atlas layout");
        let uniform_layout = |dynamic: bool, label| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: dynamic,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            })
        };
        let globals_layout = uniform_layout(true, "globals layout");
        let screen_layout = uniform_layout(false, "screen layout");

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: UNIFORM_STRIDE * MAX_PASSES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &globals,
                    offset: 0,
                    size: wgpu::BufferSize::new(80),
                }),
            }],
        });
        let screen_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screen"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let screen_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("screen"),
            layout: &screen_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: screen_uniform.as_entire_binding() }],
        });

        let atlas_layers = 2;
        let atlas = create_atlas(&device, atlas_layers);
        let sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let sampler_nearest = sampler(wgpu::FilterMode::Nearest);
        let sampler_linear = sampler(wgpu::FilterMode::Linear);
        let (atlas_nearest, atlas_linear) =
            atlas_binds(&device, &atlas_layout, &atlas, &sampler_nearest, &sampler_linear);

        let world_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world.wgsl").into()),
        });
        let screen_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("screen shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("screen.wgsl").into()),
        });
        let world_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&atlas_layout)],
            immediate_size: 0,
        });
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit layout"),
            bind_group_layouts: &[Some(&screen_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let ui_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui layout"),
            bind_group_layouts: &[Some(&screen_layout), Some(&atlas_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: Some(&blit_layout),
            vertex: wgpu::VertexState {
                module: &screen_shader,
                entry_point: Some("vs_blit"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &screen_shader,
                entry_point: Some("fs_blit"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&ui_layout),
            vertex: wgpu::VertexState {
                module: &screen_shader,
                entry_point: Some("vs_ui"),
                compilation_options: Default::default(),
                buffers: &[Some(ui_vertex_layout())],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &screen_shader,
                entry_point: Some("fs_ui"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let vb = wgpu::BufferUsages::VERTEX;
        let ib = wgpu::BufferUsages::INDEX;
        Ok(Renderer {
            dyn_vertices: DynBuffer::new(&device, "dynamic vertices", vb),
            dyn_indices: DynBuffer::new(&device, "dynamic indices", ib),
            ui_vertices: DynBuffer::new(&device, "ui vertices", vb),
            ui_indices: DynBuffer::new(&device, "ui indices", ib),
            device,
            queue,
            surface,
            config,
            linear_out,
            backend,
            atlas,
            atlas_layers,
            atlas_version: 0,
            atlas_nearest,
            atlas_linear,
            atlas_layout,
            texture_layout,
            sampler_nearest,
            sampler_linear,
            globals,
            globals_bind,
            screen_uniform,
            screen_bind,
            world_shader,
            world_layout,
            pipelines: None,
            blit_pipeline,
            ui_pipeline,
            targets: None,
            chunks: HashMap::new(),
            scratch: WorldMesh::new(),
            uniform_data: vec![0u8; UNIFORM_STRIDE as usize * MAX_PASSES],
            frame: 0,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.targets = None;
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    fn world_pipelines(&self, samples: u32) -> Pipelines {
        let make = |label, fs, blend: Option<wgpu::BlendState>, write: bool, compare| {
            self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&self.world_layout),
                vertex: wgpu::VertexState {
                    module: &self.world_shader,
                    entry_point: Some("vs_world"),
                    compilation_options: Default::default(),
                    buffers: &[Some(world_vertex_layout())],
                },
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &self.world_shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: SCENE_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Pipelines {
            samples,
            opaque: make("world opaque", "fs_opaque", None, true, wgpu::CompareFunction::LessEqual),
            blend: make(
                "world blend",
                "fs_blend",
                Some(wgpu::BlendState::ALPHA_BLENDING),
                false,
                wgpu::CompareFunction::LessEqual,
            ),
            fill: make("world fill", "fs_unlit", None, true, wgpu::CompareFunction::Always),
            on_top: make(
                "world on top",
                "fs_unlit",
                Some(wgpu::BlendState::ALPHA_BLENDING),
                false,
                wgpu::CompareFunction::Always,
            ),
        }
    }

    fn make_targets(&self, width: u32, height: u32, samples: u32) -> Targets {
        let tex = |label, format, samples, usage| {
            self.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let rt = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let color = tex("scene color", SCENE_FORMAT, 1, rt | wgpu::TextureUsages::TEXTURE_BINDING);
        let msaa = (samples > 1).then(|| tex("scene msaa", SCENE_FORMAT, samples, rt));
        let depth = tex("scene depth", DEPTH_FORMAT, samples, rt);
        let blit_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&color) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler_nearest) },
            ],
        });
        Targets { width, height, samples, color, msaa, depth, blit_bind }
    }

    fn upload_atlas(&mut self, assets: &Assets) {
        if self.atlas_version == assets.atlas_version() {
            return;
        }
        let atlas = assets.atlas();
        let pages = atlas.page_count();
        if pages > self.atlas_layers {
            // The atlas opened a page: grow the texture array (then upload everything).
            self.atlas_layers = pages.next_power_of_two();
            self.atlas = create_atlas(&self.device, self.atlas_layers);
            (self.atlas_nearest, self.atlas_linear) =
                atlas_binds(&self.device, &self.atlas_layout, &self.atlas, &self.sampler_nearest, &self.sampler_linear);
            self.atlas_version = 0;
        }
        let rects = if self.atlas_version == 0 { None } else { assets.atlas_updates_since(self.atlas_version) };
        let all: Vec<[u32; 5]>;
        let rects = match rects {
            // Only the changed rectangles.
            Some(r) => r,
            None => {
                all = (0..pages).map(|p| [p, 0, 0, atlas.size, atlas.size]).collect();
                all
            }
        };
        for [page, x, y, w, h] in rects {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: page },
                    aspect: wgpu::TextureAspect::All,
                },
                atlas.page(page),
                wgpu::TexelCopyBufferLayout {
                    offset: ((y * atlas.size + x) * 4) as u64,
                    bytes_per_row: Some(atlas.size * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
        self.atlas_version = assets.atlas_version();
    }

    /// Render one frame. Returns statistics.
    pub fn render(&mut self, frame: &FrameData, assets: &Assets, mode: RenderMode) -> Stats {
        self.frame += 1;
        let mut stats = Stats::default();
        self.upload_atlas(assets);

        let (scale, samples, pixel) = match mode {
            RenderMode::PixelArt { scale } => (scale.max(1), 1, true),
            RenderMode::HiRes { msaa } => (1, if msaa { 4 } else { 1 }, false),
        };
        let (sw, sh) = (self.config.width, self.config.height);
        let (tw, th) = (sw.div_ceil(scale), sh.div_ceil(scale));
        if self.targets.as_ref().is_none_or(|t| t.width != tw || t.height != th || t.samples != samples) {
            self.targets = Some(self.make_targets(tw, th, samples));
        }
        if self.pipelines.as_ref().is_none_or(|p| p.samples != samples) {
            self.pipelines = Some(self.world_pipelines(samples));
        }

        // Uniforms: slot 0 = identity (background fills), then one slot per pass.
        let write_globals = |data: &mut [u8], slot: usize, m: &[f32; 16], ambient: [f32; 4]| {
            let o = slot * UNIFORM_STRIDE as usize;
            data[o..o + 64].copy_from_slice(bytemuck::cast_slice(m));
            data[o + 64..o + 80].copy_from_slice(bytemuck::cast_slice(&ambient));
        };
        write_globals(&mut self.uniform_data, 0, &knight_core::glam::Mat4::IDENTITY.to_cols_array(), [1.0; 4]);
        let passes = &frame.passes[..frame.passes.len().min(MAX_PASSES - 1)];
        for (i, p) in passes.iter().enumerate() {
            write_globals(&mut self.uniform_data, i + 1, &p.view_proj.to_cols_array(), p.ambient.to_array());
        }
        let used = (passes.len() + 1) * UNIFORM_STRIDE as usize;
        self.queue.write_buffer(&self.globals, 0, &self.uniform_data[..used]);

        // Upload chunk meshes that changed.
        for p in passes {
            for c in &p.chunks {
                let key = (p.world_id, c.key);
                let entry = self.chunks.get_mut(&key);
                let stale = entry.as_ref().is_none_or(|e| e.version != c.version || !Arc::ptr_eq(&e.source, &c.opaque));
                if stale {
                    self.chunks.insert(
                        key,
                        GpuChunk {
                            version: c.version,
                            source: c.opaque.clone(),
                            opaque: GpuMesh::new(&self.device, &c.opaque),
                            transparent: GpuMesh::new(&self.device, &c.transparent),
                            last_used: self.frame,
                        },
                    );
                } else if let Some(e) = self.chunks.get_mut(&key) {
                    e.last_used = self.frame;
                }
            }
        }
        let frame_no = self.frame;
        // Drop GPU buffers of chunks not drawn for a while, and never keep more than a fixed
        // number (flying across a huge map would otherwise pile them up).
        self.chunks.retain(|_, c| frame_no - c.last_used < 600);
        const GPU_CHUNK_CAP: usize = 1536;
        if self.chunks.len() > GPU_CHUNK_CAP {
            let mut by_age: Vec<_> = self.chunks.iter().map(|(k, c)| (c.last_used, *k)).collect();
            by_age.sort_unstable();
            for (_, k) in by_age.into_iter().take(self.chunks.len() - GPU_CHUNK_CAP) {
                self.chunks.remove(&k);
            }
        }
        stats.chunks_cached = self.chunks.len() as u32;

        // Concatenate per-frame world geometry. Indices are rebased on the CPU (WebGL2 has no
        // base-vertex support).
        let all = &mut self.scratch;
        all.clear();
        let mut ranges = Vec::with_capacity(passes.len());
        let push = |all: &mut WorldMesh, m: &WorldMesh| {
            let start = all.indices.len() as u32;
            all.append(m);
            start..all.indices.len() as u32
        };
        let white = assets.region(assets.builtin.white).uv(0.5, 0.5);
        for p in passes {
            let bg = p.background.map(|(top, bottom)| {
                let (t, b) = (top.to_rgba8(), bottom.to_rgba8());
                let v = |x: f32, y: f32, color| Vertex { pos: [x, y, 0.0], uv: white, color, depth: 0.999_999 };
                let start = all.indices.len() as u32;
                all.quad([v(-1.0, 1.0, t), v(1.0, 1.0, t), v(1.0, -1.0, b), v(-1.0, -1.0, b)]);
                start..all.indices.len() as u32
            });
            let o = push(all, &p.opaque);
            let t = push(all, &p.transparent);
            let top = push(all, &p.on_top);
            ranges.push((bg, o, t, top));
        }
        self.dyn_vertices.write(&self.device, &self.queue, bytemuck::cast_slice(&all.vertices));
        self.dyn_indices.write(&self.device, &self.queue, bytemuck::cast_slice(&all.indices));

        let ui = &frame.ui;
        self.ui_vertices.write(&self.device, &self.queue, bytemuck::cast_slice(&ui.vertices));
        self.ui_indices.write(&self.device, &self.queue, bytemuck::cast_slice(&ui.indices));

        let screen = [
            sw as f32,
            sh as f32,
            (tw * scale) as f32,
            (th * scale) as f32,
            if self.linear_out { 1.0 } else { 0.0 },
            0.0,
            0.0,
            0.0,
        ];
        self.queue.write_buffer(&self.screen_uniform, 0, bytemuck::cast_slice(&screen));

        let surface_tex = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return stats;
            }
            _ => return stats,
        };
        let surface_view = surface_tex.texture.create_view(&Default::default());
        let targets = self.targets.as_ref().unwrap();
        let pipes = self.pipelines.as_ref().unwrap();
        let atlas_bind = if pixel { &self.atlas_nearest } else { &self.atlas_linear };
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let clear = frame.clear;
            let (view, resolve) = match &targets.msaa {
                Some(m) => (m, Some(&targets.color)),
                None => (&targets.color, None),
            };
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear.r as f64,
                            g: clear.g as f64,
                            b: clear.b as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_bind_group(1, atlas_bind, &[]);
            let s = scale as f32;
            for (i, (p, (bg, o, t, top))) in passes.iter().zip(&ranges).enumerate() {
                let v = p.viewport;
                let x0 = (v.x / s).clamp(0.0, tw as f32);
                let y0 = (v.y / s).clamp(0.0, th as f32);
                let x1 = ((v.x + v.w) / s).clamp(0.0, tw as f32);
                let y1 = ((v.y + v.h) / s).clamp(0.0, th as f32);
                if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
                    continue;
                }
                // WebGPU requires the viewport inside the target; cameras are expected to stay
                // within the window, so clamping only trims rounding overshoot.
                let (vx, vy) = ((v.x / s).clamp(0.0, tw as f32), (v.y / s).clamp(0.0, th as f32));
                rp.set_viewport(vx, vy, (v.w / s).min(tw as f32 - vx), (v.h / s).min(th as f32 - vy), 0.0, 1.0);
                rp.set_scissor_rect(x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32);
                if let Some(bg) = bg {
                    rp.set_pipeline(&pipes.fill);
                    rp.set_bind_group(0, &self.globals_bind, &[0]);
                    rp.set_vertex_buffer(0, self.dyn_vertices.buffer.slice(..));
                    rp.set_index_buffer(self.dyn_indices.buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(bg.clone(), 0, 0..1);
                    stats.draw_calls += 1;
                }
                let offset = ((i + 1) as u64 * UNIFORM_STRIDE) as u32;
                rp.set_bind_group(0, &self.globals_bind, &[offset]);
                for (transparent, range) in [(false, o), (true, t)] {
                    rp.set_pipeline(if transparent { &pipes.blend } else { &pipes.opaque });
                    for c in &p.chunks {
                        let Some(g) = self.chunks.get(&(p.world_id, c.key)) else { continue };
                        let mesh = if transparent { &g.transparent } else { &g.opaque };
                        if let Some(m) = mesh {
                            rp.set_vertex_buffer(0, m.vertices.slice(..));
                            rp.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
                            rp.draw_indexed(0..m.count, 0, 0..1);
                            stats.draw_calls += 1;
                            stats.triangles += m.count / 3;
                        }
                    }
                    if !range.is_empty() {
                        rp.set_vertex_buffer(0, self.dyn_vertices.buffer.slice(..));
                        rp.set_index_buffer(self.dyn_indices.buffer.slice(..), wgpu::IndexFormat::Uint32);
                        rp.draw_indexed(range.clone(), 0, 0..1);
                        stats.draw_calls += 1;
                        stats.triangles += (range.end - range.start) / 3;
                    }
                }
                if !top.is_empty() {
                    rp.set_pipeline(&pipes.on_top);
                    rp.set_vertex_buffer(0, self.dyn_vertices.buffer.slice(..));
                    rp.set_index_buffer(self.dyn_indices.buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(top.clone(), 0, 0..1);
                    stats.draw_calls += 1;
                    stats.triangles += (top.end - top.start) / 3;
                }
                stats.chunks_drawn += p.chunks.len() as u32;
            }
        }
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("screen"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_bind_group(0, &self.screen_bind, &[]);
            rp.set_pipeline(&self.blit_pipeline);
            rp.set_bind_group(1, &targets.blit_bind, &[]);
            rp.draw(0..3, 0..1);
            stats.draw_calls += 1;
            if !ui.indices.is_empty() {
                rp.set_pipeline(&self.ui_pipeline);
                rp.set_bind_group(1, &self.atlas_nearest, &[]);
                rp.set_vertex_buffer(0, self.ui_vertices.buffer.slice(..));
                rp.set_index_buffer(self.ui_indices.buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..ui.indices.len() as u32, 0, 0..1);
                stats.draw_calls += 1;
                stats.triangles += ui.indices.len() as u32 / 3;
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(surface_tex);
        stats
    }
}

/// The atlas texture array. Always at least two layers: WebGL2 can't tell a one-layer array
/// from a plain 2D texture.
fn create_atlas(device: &wgpu::Device, layers: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("atlas"),
        size: wgpu::Extent3d { width: 2048, height: 2048, depth_or_array_layers: layers.max(2) },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Nearest and linear bind groups for the atlas array.
fn atlas_binds(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    atlas: &wgpu::Texture,
    nearest: &wgpu::Sampler,
    linear: &wgpu::Sampler,
) -> (wgpu::BindGroup, wgpu::BindGroup) {
    let view = atlas.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let bind = |s: &wgpu::Sampler| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(s) },
            ],
        })
    };
    (bind(nearest), bind(linear))
}
