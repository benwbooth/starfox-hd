//! wgpu rendering backend — replaces the glow GL backend.
//!
//! The whole renderer is two shader programs (see `gl_backend.rs` history):
//! `flat` (3D: position + proj/view/model + solid color) and `overlay`
//! (2D: position + uv, textured with solid / RGBA / palette-indexed modes).
//! This module exposes a small *retained* draw API — passes push vertices
//! and per-draw state into CPU buffers during a frame; `end_frame` uploads
//! everything once and replays it in a single render pass, preserving the
//! call order (3D first, 2D overlays after) that the immediate-mode GL code
//! relied on.
//!
//! WGSL faithfully mirrors the GLSL in `gl_backend.rs`:
//! - flat vs: `proj * view * model * vec4(pos, 1)`; fs: `uColor`.
//! - overlay vs: `proj * model * vec4(pos, 0, 1)`; fs: mode 2 palette
//!   (discard index 0), mode 1 RGBA (discard a<0.5), mode 0 solid color.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Depth buffer format (matches the old `set_depth_size(24)`).
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

/// 3D vertex for the `flat` pipeline.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex3 {
    pub pos: [f32; 3],
}

/// 3D position plus the inputs to the Super FX texture address calculation.
/// `tex_info` is `[local_x, local_y, base_address, wrap_mask]`; keeping the
/// address calculation in the fragment shader preserves row carry and the
/// per-layout masks used by `MDRAWP.MC`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex3Tex {
    pub pos: [f32; 3],
    pub tex_info: [f32; 4],
}

/// 2D vertex for the `overlay` pipeline (matches HUD `aPos`/`aTexCoord`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex2 {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
}

/// Per-draw uniform block, shared by both pipelines (dynamic-offset buffer).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DrawUniform {
    proj: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    color: [f32; 4],
    /// Pipeline-specific typed mode and compact parameters.
    mode: [u32; 4],
    palette: [[f32; 4]; 16],
}
// size = 3*64 + 16 + 16 + 256 = 480 bytes; dynamic offsets need 256 alignment.
const UNIFORM_STRIDE: u64 = 512;
const FLAT_FILL_SOLID: u32 = 0;
const FLAT_FILL_PALETTE_PAIR: u32 = 1;
const DISPLAY_BRIGHTNESS_MAX: u8 = 15;
const DISPLAY_SCALE_DENOMINATOR: u32 = DISPLAY_BRIGHTNESS_MAX as u32;

fn identity() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mat4(m: &[f32; 16]) -> [[f32; 4]; 4] {
    [
        [m[0], m[1], m[2], m[3]],
        [m[4], m[5], m[6], m[7]],
        [m[8], m[9], m[10], m[11]],
        [m[12], m[13], m[14], m[15]],
    ]
}

/// Opaque handle into the texture cache. `WHITE_TEX` (id 0) is a 1x1 white
/// texture so solid-mode overlay draws always have a valid bind group.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextureId(pub usize);
pub const WHITE_TEX: TextureId = TextureId(0);

#[derive(Clone, Copy, PartialEq)]
enum Pipe {
    FlatTri,
    TexturedTri,
    /// Alpha-blended, depth-tested but NOT depth-writing (ground shadows).
    FlatTriAlpha,
    /// Additive-blended, depth-tested but NOT depth-writing (particles).
    FlatTriAdd,
    FlatLine,
    Overlay,
    /// Additive two-dimensional fixed-color pass (retail color math).
    OverlayAdd,
}

struct DrawCmd {
    pipe: Pipe,
    v_start: u32,
    v_count: u32,
    uniform_index: u32,
    texture: TextureId,
    viewport: Option<RenderViewport>,
}

/// Output rectangle captured by retained draw commands. This is presentation
/// state rather than game state: native objects remain in one flat world while
/// the renderer confines a source frame to its intended display area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderViewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

struct CachedTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    /// bytes per pixel of the stored format (1 = R8, 4 = RGBA8).
    bpp: u32,
}

/// Where a frame is rendered.
enum Target {
    Surface {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    /// Headless render-to-texture (tests). RGBA8, COPY_SRC for readback.
    Offscreen {
        texture: wgpu::Texture,
        width: u32,
        height: u32,
    },
}

/// In-flight surface frame, held between begin/end.
struct SurfaceFrame {
    surface_tex: wgpu::SurfaceTexture,
}

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: Target,

    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),

    flat_tri: wgpu::RenderPipeline,
    flat_tri_alpha: wgpu::RenderPipeline,
    flat_tri_add: wgpu::RenderPipeline,
    flat_line: wgpu::RenderPipeline,
    textured_tri: wgpu::RenderPipeline,
    overlay: wgpu::RenderPipeline,
    overlay_add: wgpu::RenderPipeline,
    display_brightness: wgpu::RenderPipeline,

    uniform_bgl: wgpu::BindGroupLayout,
    texture_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    repeat_sampler: wgpu::Sampler,
    color_format: wgpu::TextureFormat,
    presentation_texture: wgpu::Texture,
    presentation_view: wgpu::TextureView,
    presentation_bind: wgpu::BindGroup,
    presentation_scale_numerator: u32,
    presentation_black_subtraction: u32,

    // Persistent GPU buffers (grown on demand).
    vbuf3: wgpu::Buffer,
    vbuf3_cap: u64,
    vbuf3t: wgpu::Buffer,
    vbuf3t_cap: u64,
    vbuf2: wgpu::Buffer,
    vbuf2_cap: u64,
    ubuf: wgpu::Buffer,
    ubuf_cap: u64,
    uniform_bind: wgpu::BindGroup,

    textures: Vec<CachedTexture>,

    // Per-frame CPU-side accumulation.
    v3: Vec<Vertex3>,
    v3t: Vec<Vertex3Tex>,
    v2: Vec<Vertex2>,
    uniforms: Vec<DrawUniform>,
    cmds: Vec<DrawCmd>,
    draw_viewport: Option<RenderViewport>,
    clear: [f64; 4],

    pending_surface: Option<SurfaceFrame>,
}

impl Gpu {
    /// Construct from a caller-created instance + surface (sf-app owns the
    /// SDL3 window and builds the surface via raw-window-handle).
    pub fn new_for_surface(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .map_err(|e| format!("no wgpu adapter: {e}"))?;

        let (device, queue) = Self::request_device(&adapter)?;

        let caps = surface.get_capabilities(&adapter);
        // Prefer a non-sRGB 8-bit format so colors are written verbatim like
        // the GL path (the shaders do no gamma conversion).
        let color_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                )
            })
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: color_format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self::finish_init(
            device,
            queue,
            Target::Surface { surface, config },
            color_format,
            width.max(1),
            height.max(1),
        ))
    }

    /// Headless render-to-texture backend for offscreen tests.
    pub fn new_headless(width: u32, height: u32) -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|e| format!("no wgpu adapter: {e}"))?;
        let (device, queue) = Self::request_device(&adapter)?;

        let color_format = wgpu::TextureFormat::Rgba8Unorm;
        let (w, h) = (width.max(1), height.max(1));
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen-color"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Ok(Self::finish_init(
            device,
            queue,
            Target::Offscreen {
                texture,
                width: w,
                height: h,
            },
            color_format,
            w,
            h,
        ))
    }

    fn request_device(adapter: &wgpu::Adapter) -> Result<(wgpu::Device, wgpu::Queue), String> {
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("sf-render device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("request_device failed: {e}"))
    }

    fn finish_init(
        device: wgpu::Device,
        queue: wgpu::Queue,
        target: Target,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sf-render wgsl"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<DrawUniform>() as u64
                    ),
                },
                count: None,
            }],
        });
        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
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
        });

        let flat_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("flat-layout"),
            bind_group_layouts: &[&uniform_bgl],
            push_constant_ranges: &[],
        });
        let overlay_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay-layout"),
            bind_group_layouts: &[&uniform_bgl, &texture_bgl],
            push_constant_ranges: &[],
        });

        let depth_stencil = |write: bool, compare: wgpu::CompareFunction| {
            Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: write,
                depth_compare: compare,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            })
        };

        let v3_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex3>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            }],
        };
        let v3t_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex3Tex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 12,
                    shader_location: 1,
                },
            ],
        };
        let v2_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex2>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
            ],
        };

        // Additive blend for particles (SRC_ALPHA, ONE — matches the old
        // glBlendFunc(GL_SRC_ALPHA, GL_ONE)).
        let additive_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };

        let make_flat = |topology: wgpu::PrimitiveTopology,
                         blend: Option<wgpu::BlendState>,
                         depth_write: bool,
                         label: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&flat_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_flat"),
                    compilation_options: Default::default(),
                    buffers: &[v3_layout.clone()],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: depth_stencil(depth_write, wgpu::CompareFunction::Less),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_flat"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let flat_tri = make_flat(
            wgpu::PrimitiveTopology::TriangleList,
            None,
            true,
            "flat-tri",
        );
        let flat_line = make_flat(wgpu::PrimitiveTopology::LineList, None, true, "flat-line");
        // Shadows: alpha-blended, depth-tested, depth-write OFF.
        let flat_tri_alpha = make_flat(
            wgpu::PrimitiveTopology::TriangleList,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            false,
            "flat-tri-alpha",
        );
        // Particles: additive, depth-tested, depth-write OFF.
        let flat_tri_add = make_flat(
            wgpu::PrimitiveTopology::TriangleList,
            Some(additive_blend),
            false,
            "flat-tri-add",
        );

        let textured_tri = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("textured-tri"),
            layout: Some(&overlay_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_textured"),
                compilation_options: Default::default(),
                buffers: &[v3t_layout],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            // COLTEXT faces are authored as coplanar overlays immediately
            // after a solid backing face (for example the two Corneria-base
            // insignias in mybase_0). `Less` made the equal-depth texture lose
            // nondeterministically as the camera moved. Preserve source draw
            // order by allowing the later textured decal at equal depth.
            depth_stencil: depth_stencil(true, wgpu::CompareFunction::LessEqual),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_textured"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        let make_overlay = |label: &str, blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&overlay_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_overlay"),
                    compilation_options: Default::default(),
                    buffers: &[v2_layout.clone()],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                // 2D overlays draw back-to-front in call order; no depth testing.
                depth_stencil: depth_stencil(false, wgpu::CompareFunction::Always),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_overlay"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let overlay = make_overlay("overlay", wgpu::BlendState::ALPHA_BLENDING);
        let overlay_add = make_overlay("overlay-add", additive_blend);
        let display_brightness = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("display-brightness"),
            layout: Some(&overlay_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_display_brightness"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_display_brightness"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest-clamp"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let repeat_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest-repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let depth_view = make_depth(&device, width, height);
        let (presentation_texture, presentation_view, presentation_bind) =
            make_presentation_target(&device, &texture_bgl, &sampler, color_format, width, height);

        // Initial buffers (small; grown on demand).
        let vbuf3 = make_vbuf(&device, 4096 * std::mem::size_of::<Vertex3>() as u64);
        let vbuf3t = make_vbuf(&device, 4096 * std::mem::size_of::<Vertex3Tex>() as u64);
        let vbuf2 = make_vbuf(&device, 4096 * std::mem::size_of::<Vertex2>() as u64);
        let ubuf_cap = 256 * UNIFORM_STRIDE;
        let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: ubuf_cap,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind = make_uniform_bind(&device, &uniform_bgl, &ubuf);

        let mut gpu = Self {
            device,
            queue,
            target,
            depth_view,
            depth_size: (width, height),
            flat_tri,
            flat_tri_alpha,
            flat_tri_add,
            flat_line,
            textured_tri,
            overlay,
            overlay_add,
            display_brightness,
            uniform_bgl,
            texture_bgl,
            sampler,
            repeat_sampler,
            color_format,
            presentation_texture,
            presentation_view,
            presentation_bind,
            presentation_scale_numerator: DISPLAY_SCALE_DENOMINATOR,
            presentation_black_subtraction: 0,
            vbuf3,
            vbuf3_cap: 4096 * std::mem::size_of::<Vertex3>() as u64,
            vbuf3t,
            vbuf3t_cap: 4096 * std::mem::size_of::<Vertex3Tex>() as u64,
            vbuf2,
            vbuf2_cap: 4096 * std::mem::size_of::<Vertex2>() as u64,
            ubuf,
            ubuf_cap,
            uniform_bind,
            textures: Vec::new(),
            v3: Vec::new(),
            v3t: Vec::new(),
            v2: Vec::new(),
            uniforms: Vec::new(),
            cmds: Vec::new(),
            draw_viewport: None,
            clear: [0.0, 0.0, 0.0, 1.0],
            pending_surface: None,
        };
        // id 0 = 1x1 white for solid overlay draws.
        gpu.create_texture_rgba(1, 1, &[255, 255, 255, 255]);
        gpu
    }

    pub fn size(&self) -> (u32, u32) {
        match &self.target {
            Target::Surface { config, .. } => (config.width, config.height),
            Target::Offscreen { width, height, .. } => (*width, *height),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (w, h) = (width.max(1), height.max(1));
        match &mut self.target {
            Target::Surface { surface, config } => {
                config.width = w;
                config.height = h;
                surface.configure(&self.device, config);
            }
            Target::Offscreen { .. } => {}
        }
        if self.depth_size != (w, h) {
            self.depth_view = make_depth(&self.device, w, h);
            self.depth_size = (w, h);
            let (texture, view, bind) = make_presentation_target(
                &self.device,
                &self.texture_bgl,
                &self.sampler,
                self.color_format,
                w,
                h,
            );
            self.presentation_texture = texture;
            self.presentation_view = view;
            self.presentation_bind = bind;
        }
    }

    /// Select the source display's typed presentation state. Brightness levels
    /// zero through 15 scale five-bit components by `level / 15`; scene
    /// transfer blanking is a separate all-black state.
    pub fn set_display_presentation(
        &mut self,
        level: u8,
        forced_blank: bool,
        black_subtraction: u8,
    ) {
        let level = level.min(DISPLAY_BRIGHTNESS_MAX);
        self.presentation_scale_numerator = if forced_blank { 0 } else { u32::from(level) };
        self.presentation_black_subtraction = u32::from(black_subtraction.min(31));
    }

    // ---- Texture cache ------------------------------------------------------

    /// Upload an RGBA8 texture, returns its id.
    pub fn create_texture_rgba(&mut self, width: u32, height: u32, data: &[u8]) -> TextureId {
        self.create_texture(
            width,
            height,
            4,
            wgpu::TextureFormat::Rgba8Unorm,
            data,
            false,
        )
    }

    /// Upload an RGBA8 texture whose authored coordinates wrap like a tilemap.
    pub fn create_texture_rgba_repeat(
        &mut self,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> TextureId {
        self.create_texture(
            width,
            height,
            4,
            wgpu::TextureFormat::Rgba8Unorm,
            data,
            true,
        )
    }

    /// Upload a single-channel R8 texture (font atlas / palette indices).
    pub fn create_texture_r8(&mut self, width: u32, height: u32, data: &[u8]) -> TextureId {
        self.create_texture(
            width,
            height,
            1,
            wgpu::TextureFormat::R8Unorm,
            data,
            false,
        )
    }

    fn create_texture(
        &mut self,
        width: u32,
        height: u32,
        bpp: u32,
        format: wgpu::TextureFormat,
        data: &[u8],
        repeat: bool,
    ) -> TextureId {
        let (w, h) = (width.max(1), height.max(1));
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tex"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tex-bind"),
            layout: &self.texture_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(if repeat {
                        &self.repeat_sampler
                    } else {
                        &self.sampler
                    }),
                },
            ],
        });
        let id = TextureId(self.textures.len());
        self.textures.push(CachedTexture {
            texture,
            bind_group,
            width: w,
            height: h,
            bpp,
        });
        self.write_texture(id, data);
        id
    }

    /// Re-upload pixel data to an existing texture (same dimensions/format).
    pub fn update_texture(&mut self, id: TextureId, data: &[u8]) {
        self.write_texture(id, data);
    }

    fn write_texture(&self, id: TextureId, data: &[u8]) {
        let t = &self.textures[id.0];
        let bytes_per_row = t.width * t.bpp;
        let needed = (bytes_per_row * t.height) as usize;
        if data.len() < needed {
            return;
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &t.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data[..needed],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(t.height),
            },
            wgpu::Extent3d {
                width: t.width,
                height: t.height,
                depth_or_array_layers: 1,
            },
        );
    }

    // ---- Frame API ----------------------------------------------------------

    pub fn set_clear_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.clear = [r as f64, g as f64, b as f64, a as f64];
    }

    /// Begin a frame: reset accumulation and acquire the surface texture.
    pub fn begin_frame(&mut self) {
        self.v3.clear();
        self.v3t.clear();
        self.v2.clear();
        self.uniforms.clear();
        self.cmds.clear();
        self.draw_viewport = None;
        if let Target::Surface { surface, .. } = &self.target {
            match surface.get_current_texture() {
                Ok(surface_tex) => {
                    self.pending_surface = Some(SurfaceFrame { surface_tex });
                }
                Err(_) => {
                    // Surface lost/outdated: reconfigure and skip this frame.
                    if let Target::Surface { surface, config } = &self.target {
                        surface.configure(&self.device, config);
                    }
                    self.pending_surface = None;
                }
            }
        }
    }

    /// Select the viewport captured by subsequently queued commands. Passing
    /// `None` restores the full render target for HUD and other overlays.
    pub fn set_draw_viewport(&mut self, viewport: Option<RenderViewport>) {
        self.draw_viewport = viewport;
    }

    fn push_uniform(&mut self, u: DrawUniform) -> u32 {
        let idx = self.uniforms.len() as u32;
        self.uniforms.push(u);
        idx
    }

    /// Draw solid-color 3D triangles (flat pipeline).
    pub fn push_flat_tris(
        &mut self,
        verts: &[Vertex3],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        color: [f32; 4],
    ) {
        if verts.len() < 3 {
            return;
        }
        let ui = self.push_uniform(DrawUniform {
            proj: mat4(proj),
            view: mat4(view),
            model: mat4(model),
            color,
            mode: [FLAT_FILL_SOLID, 0, 0, 0],
            palette: [[0.0; 4]; 16],
        });
        let start = self.v3.len() as u32;
        self.v3.extend_from_slice(verts);
        self.cmds.push(DrawCmd {
            pipe: Pipe::FlatTri,
            v_start: start,
            v_count: verts.len() as u32,
            uniform_index: ui,
            texture: WHITE_TEX,
            viewport: self.draw_viewport,
        });
    }

    /// Draw flat 3D triangles using the retail two-color checkerboard. The
    /// pattern is evaluated in the 256x224 source raster and therefore stays
    /// stable when the presentation viewport is enlarged.
    #[allow(clippy::too_many_arguments)]
    pub fn push_palette_pair_tris(
        &mut self,
        verts: &[Vertex3],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        palette: &[[f32; 4]; 16],
        pair: [u8; 2],
    ) {
        self.push_palette_pair_variant(Pipe::FlatTri, verts, proj, view, model, palette, pair);
    }

    /// Draw a retail two-color checkerboard without writing depth. Color zero
    /// remains transparent, making this suitable for optional dithered ground
    /// shadows in the HD presentation.
    #[allow(clippy::too_many_arguments)]
    pub fn push_palette_pair_tris_alpha(
        &mut self,
        verts: &[Vertex3],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        palette: &[[f32; 4]; 16],
        pair: [u8; 2],
    ) {
        self.push_palette_pair_variant(Pipe::FlatTriAlpha, verts, proj, view, model, palette, pair);
    }

    /// Draw palette-indexed Super FX texture-map triangles in 3D.  Each R8
    /// texel stores both source CGX planes; mode 2 selects its low nibble and
    /// mode 3 the high nibble, exactly matching the source texture selector.
    #[allow(clippy::too_many_arguments)]
    pub fn push_textured_tris(
        &mut self,
        verts: &[Vertex3Tex],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        palette: &[[f32; 4]; 16],
        high_nibble: bool,
        texture: TextureId,
    ) {
        if verts.len() < 3 {
            return;
        }
        let ui = self.push_uniform(DrawUniform {
            proj: mat4(proj),
            view: mat4(view),
            model: mat4(model),
            color: [1.0; 4],
            mode: [if high_nibble { 3 } else { 2 }, 0, 0, 0],
            palette: *palette,
        });
        let start = self.v3t.len() as u32;
        self.v3t.extend_from_slice(verts);
        self.cmds.push(DrawCmd {
            pipe: Pipe::TexturedTri,
            v_start: start,
            v_count: verts.len() as u32,
            uniform_index: ui,
            texture,
            viewport: self.draw_viewport,
        });
    }

    /// Alpha-blended, non-depth-writing 3D triangles (ground shadows).
    pub fn push_flat_tris_alpha(
        &mut self,
        verts: &[Vertex3],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        color: [f32; 4],
    ) {
        self.push_flat_variant(Pipe::FlatTriAlpha, verts, proj, view, model, color);
    }

    /// Additive-blended, non-depth-writing 3D triangles (particles).
    pub fn push_flat_tris_add(
        &mut self,
        verts: &[Vertex3],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        color: [f32; 4],
    ) {
        self.push_flat_variant(Pipe::FlatTriAdd, verts, proj, view, model, color);
    }

    fn push_flat_variant(
        &mut self,
        pipe: Pipe,
        verts: &[Vertex3],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        color: [f32; 4],
    ) {
        if verts.len() < 3 {
            return;
        }
        let ui = self.push_uniform(DrawUniform {
            proj: mat4(proj),
            view: mat4(view),
            model: mat4(model),
            color,
            mode: [FLAT_FILL_SOLID, 0, 0, 0],
            palette: [[0.0; 4]; 16],
        });
        let start = self.v3.len() as u32;
        self.v3.extend_from_slice(verts);
        self.cmds.push(DrawCmd {
            pipe,
            v_start: start,
            v_count: verts.len() as u32,
            uniform_index: ui,
            texture: WHITE_TEX,
            viewport: self.draw_viewport,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn push_palette_pair_variant(
        &mut self,
        pipe: Pipe,
        verts: &[Vertex3],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        palette: &[[f32; 4]; 16],
        pair: [u8; 2],
    ) {
        if verts.len() < 2 {
            return;
        }
        let raster = self.draw_viewport.unwrap_or(RenderViewport {
            x: 0,
            y: 0,
            width: self.depth_size.0,
            height: self.depth_size.1,
        });
        let ui = self.push_uniform(DrawUniform {
            proj: mat4(proj),
            view: mat4(view),
            model: mat4(model),
            color: [
                raster.x as f32,
                raster.y as f32,
                raster.width as f32,
                raster.height as f32,
            ],
            mode: [
                FLAT_FILL_PALETTE_PAIR,
                u32::from(pair[0]),
                u32::from(pair[1]),
                0,
            ],
            palette: *palette,
        });
        let start = self.v3.len() as u32;
        self.v3.extend_from_slice(verts);
        self.cmds.push(DrawCmd {
            pipe,
            v_start: start,
            v_count: verts.len() as u32,
            uniform_index: ui,
            texture: WHITE_TEX,
            viewport: self.draw_viewport,
        });
    }

    /// Draw solid-color 3D lines (flat pipeline). `positions` is xyz triples.
    pub fn push_flat_lines(
        &mut self,
        verts: &[Vertex3],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        color: [f32; 4],
    ) {
        let n = verts.len() & !1;
        if n < 2 {
            return;
        }
        let ui = self.push_uniform(DrawUniform {
            proj: mat4(proj),
            view: mat4(view),
            model: mat4(model),
            color,
            mode: [FLAT_FILL_SOLID, 0, 0, 0],
            palette: [[0.0; 4]; 16],
        });
        let start = self.v3.len() as u32;
        self.v3.extend_from_slice(&verts[..n]);
        self.cmds.push(DrawCmd {
            pipe: Pipe::FlatLine,
            v_start: start,
            v_count: n as u32,
            uniform_index: ui,
            texture: WHITE_TEX,
            viewport: self.draw_viewport,
        });
    }

    /// Draw a retail dithered two-color line in the source raster.
    #[allow(clippy::too_many_arguments)]
    pub fn push_palette_pair_lines(
        &mut self,
        verts: &[Vertex3],
        proj: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        palette: &[[f32; 4]; 16],
        pair: [u8; 2],
    ) {
        let n = verts.len() & !1;
        if n < 2 {
            return;
        }
        self.push_palette_pair_variant(
            Pipe::FlatLine,
            &verts[..n],
            proj,
            view,
            model,
            palette,
            pair,
        );
    }

    /// Draw 2D overlay triangles. `use_texture`: 0 solid, 1 rgba, 2 palette.
    #[allow(clippy::too_many_arguments)]
    pub fn push_overlay_tris(
        &mut self,
        verts: &[Vertex2],
        proj: &[f32; 16],
        model: &[f32; 16],
        color: [f32; 4],
        use_texture: u32,
        palette: Option<&[[f32; 4]; 16]>,
        texture: TextureId,
    ) {
        self.push_overlay_tris_variant(
            Pipe::Overlay,
            verts,
            proj,
            model,
            color,
            use_texture,
            palette,
            texture,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn push_overlay_tris_variant(
        &mut self,
        pipe: Pipe,
        verts: &[Vertex2],
        proj: &[f32; 16],
        model: &[f32; 16],
        color: [f32; 4],
        use_texture: u32,
        palette: Option<&[[f32; 4]; 16]>,
        texture: TextureId,
    ) {
        if verts.len() < 3 {
            return;
        }
        let ui = self.push_uniform(DrawUniform {
            proj: mat4(proj),
            view: identity(),
            model: mat4(model),
            color,
            mode: [use_texture, 0, 0, 0],
            palette: palette.copied().unwrap_or([[0.0; 4]; 16]),
        });
        let start = self.v2.len() as u32;
        self.v2.extend_from_slice(verts);
        self.cmds.push(DrawCmd {
            pipe,
            v_start: start,
            v_count: verts.len() as u32,
            uniform_index: ui,
            texture,
            viewport: self.draw_viewport,
        });
    }

    /// Draw a 2D overlay triangle *fan* (v0,v1,v2, v0,v2,v3, ...). wgpu has
    /// no fan topology, so it's expanded to a triangle list here.
    #[allow(clippy::too_many_arguments)]
    pub fn push_overlay_fan(
        &mut self,
        fan: &[Vertex2],
        proj: &[f32; 16],
        model: &[f32; 16],
        color: [f32; 4],
        use_texture: u32,
        palette: Option<&[[f32; 4]; 16]>,
        texture: TextureId,
    ) {
        if fan.len() < 3 {
            return;
        }
        let mut tris = Vec::with_capacity((fan.len() - 2) * 3);
        for i in 1..fan.len() - 1 {
            tris.push(fan[0]);
            tris.push(fan[i]);
            tris.push(fan[i + 1]);
        }
        self.push_overlay_tris(&tris, proj, model, color, use_texture, palette, texture);
    }

    /// Draw a solid two-dimensional triangle fan with additive color math.
    pub fn push_overlay_additive_fan(
        &mut self,
        fan: &[Vertex2],
        proj: &[f32; 16],
        model: &[f32; 16],
        color: [f32; 4],
    ) {
        if fan.len() < 3 {
            return;
        }
        let mut tris = Vec::with_capacity((fan.len() - 2) * 3);
        for point in 1..fan.len() - 1 {
            tris.push(fan[0]);
            tris.push(fan[point]);
            tris.push(fan[point + 1]);
        }
        self.push_overlay_tris_variant(
            Pipe::OverlayAdd,
            &tris,
            proj,
            model,
            color,
            0,
            None,
            WHITE_TEX,
        );
    }

    /// Upload accumulated geometry and replay all draws in one render pass.
    pub fn end_frame(&mut self) {
        // Surface frames without an acquired texture are dropped.
        let color_view: wgpu::TextureView = match &self.target {
            Target::Surface { .. } => match self.pending_surface.as_ref() {
                Some(f) => f
                    .surface_tex
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default()),
                None => return,
            },
            Target::Offscreen { texture, .. } => {
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            }
        };

        let apply_display_presentation = self.presentation_scale_numerator
            < DISPLAY_SCALE_DENOMINATOR
            || self.presentation_black_subtraction != 0;
        let display_uniform_index = apply_display_presentation.then(|| {
            self.push_uniform(DrawUniform {
                proj: identity(),
                view: identity(),
                model: identity(),
                color: [1.0; 4],
                mode: [
                    self.presentation_scale_numerator,
                    self.presentation_black_subtraction,
                    0,
                    0,
                ],
                palette: [[0.0; 4]; 16],
            })
        });
        self.upload_frame_buffers();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame-encoder"),
            });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if apply_display_presentation {
                        &self.presentation_view
                    } else {
                        &color_view
                    },
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear[0],
                            g: self.clear[1],
                            b: self.clear[2],
                            a: self.clear[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            for cmd in &self.cmds {
                let offset = (cmd.uniform_index as u64) * UNIFORM_STRIDE;
                if let Some(viewport) = cmd.viewport {
                    rpass.set_viewport(
                        viewport.x as f32,
                        viewport.y as f32,
                        viewport.width as f32,
                        viewport.height as f32,
                        0.0,
                        1.0,
                    );
                    rpass.set_scissor_rect(viewport.x, viewport.y, viewport.width, viewport.height);
                } else {
                    let (width, height) = self.depth_size;
                    rpass.set_viewport(0.0, 0.0, width as f32, height as f32, 0.0, 1.0);
                    rpass.set_scissor_rect(0, 0, width, height);
                }
                match cmd.pipe {
                    Pipe::FlatTri | Pipe::FlatTriAlpha | Pipe::FlatTriAdd | Pipe::FlatLine => {
                        rpass.set_pipeline(match cmd.pipe {
                            Pipe::FlatTri => &self.flat_tri,
                            Pipe::FlatTriAlpha => &self.flat_tri_alpha,
                            Pipe::FlatTriAdd => &self.flat_tri_add,
                            _ => &self.flat_line,
                        });
                        rpass.set_bind_group(0, &self.uniform_bind, &[offset as u32]);
                        rpass.set_vertex_buffer(0, self.vbuf3.slice(..));
                        rpass.draw(cmd.v_start..cmd.v_start + cmd.v_count, 0..1);
                    }
                    Pipe::TexturedTri => {
                        rpass.set_pipeline(&self.textured_tri);
                        rpass.set_bind_group(0, &self.uniform_bind, &[offset as u32]);
                        rpass.set_bind_group(1, &self.textures[cmd.texture.0].bind_group, &[]);
                        rpass.set_vertex_buffer(0, self.vbuf3t.slice(..));
                        rpass.draw(cmd.v_start..cmd.v_start + cmd.v_count, 0..1);
                    }
                    Pipe::Overlay | Pipe::OverlayAdd => {
                        rpass.set_pipeline(if cmd.pipe == Pipe::Overlay {
                            &self.overlay
                        } else {
                            &self.overlay_add
                        });
                        rpass.set_bind_group(0, &self.uniform_bind, &[offset as u32]);
                        rpass.set_bind_group(1, &self.textures[cmd.texture.0].bind_group, &[]);
                        rpass.set_vertex_buffer(0, self.vbuf2.slice(..));
                        rpass.draw(cmd.v_start..cmd.v_start + cmd.v_count, 0..1);
                    }
                }
            }
        }
        if let Some(uniform_index) = display_uniform_index {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("display-brightness-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&self.display_brightness);
            rpass.set_bind_group(
                0,
                &self.uniform_bind,
                &[(u64::from(uniform_index) * UNIFORM_STRIDE) as u32],
            );
            rpass.set_bind_group(1, &self.presentation_bind, &[]);
            rpass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));

        if let Some(frame) = self.pending_surface.take() {
            frame.surface_tex.present();
        }
    }

    fn upload_frame_buffers(&mut self) {
        // Grow vertex/uniform buffers if the frame exceeded capacity.
        let need3 = (self.v3.len() * std::mem::size_of::<Vertex3>()) as u64;
        if need3 > self.vbuf3_cap {
            self.vbuf3_cap = (need3 * 2).max(self.vbuf3_cap * 2);
            self.vbuf3 = make_vbuf(&self.device, self.vbuf3_cap);
        }
        let need3t = (self.v3t.len() * std::mem::size_of::<Vertex3Tex>()) as u64;
        if need3t > self.vbuf3t_cap {
            self.vbuf3t_cap = (need3t * 2).max(self.vbuf3t_cap * 2);
            self.vbuf3t = make_vbuf(&self.device, self.vbuf3t_cap);
        }
        let need2 = (self.v2.len() * std::mem::size_of::<Vertex2>()) as u64;
        if need2 > self.vbuf2_cap {
            self.vbuf2_cap = (need2 * 2).max(self.vbuf2_cap * 2);
            self.vbuf2 = make_vbuf(&self.device, self.vbuf2_cap);
        }
        let needu = self.uniforms.len() as u64 * UNIFORM_STRIDE;
        if needu > self.ubuf_cap {
            self.ubuf_cap = (needu * 2).max(self.ubuf_cap * 2);
            self.ubuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("uniforms"),
                size: self.ubuf_cap,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.uniform_bind = make_uniform_bind(&self.device, &self.uniform_bgl, &self.ubuf);
        }
        if !self.v3.is_empty() {
            self.queue
                .write_buffer(&self.vbuf3, 0, bytemuck::cast_slice(&self.v3));
        }
        if !self.v3t.is_empty() {
            self.queue
                .write_buffer(&self.vbuf3t, 0, bytemuck::cast_slice(&self.v3t));
        }
        if !self.v2.is_empty() {
            self.queue
                .write_buffer(&self.vbuf2, 0, bytemuck::cast_slice(&self.v2));
        }
        // Uniforms are laid out at UNIFORM_STRIDE intervals for dynamic offset.
        if !self.uniforms.is_empty() {
            let mut bytes = vec![0u8; self.uniforms.len() * UNIFORM_STRIDE as usize];
            let usize_of = std::mem::size_of::<DrawUniform>();
            for (i, u) in self.uniforms.iter().enumerate() {
                let off = i * UNIFORM_STRIDE as usize;
                bytes[off..off + usize_of].copy_from_slice(bytemuck::bytes_of(u));
            }
            self.queue.write_buffer(&self.ubuf, 0, &bytes);
        }
    }

    /// Read the offscreen color texture back as RGBA8 rows (tests only).
    pub fn read_pixels(&self) -> Option<(u32, u32, Vec<u8>)> {
        let (texture, width, height) = match &self.target {
            Target::Offscreen {
                texture,
                width,
                height,
                ..
            } => (texture, *width, *height),
            _ => return None,
        };
        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback-encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::Wait);
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((unpadded * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            out.extend_from_slice(&data[start..start + unpadded as usize]);
        }
        drop(data);
        buffer.unmap();
        Some((width, height, out))
    }
}

fn make_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn make_presentation_target(
    device: &wgpu::Device,
    texture_bgl: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("presentation-color"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("presentation-bind"),
        layout: texture_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (texture, view, bind)
}

fn make_vbuf(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vbuf"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn make_uniform_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("uniform-bind"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<DrawUniform>() as u64),
            }),
        }],
    })
}

// Kept for buffer-init symmetry; not all builds use it.
#[allow(dead_code)]
fn init_buffer(device: &wgpu::Device, data: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: data,
        usage,
    })
}

const WGSL: &str = r#"
struct Uniforms {
    proj: mat4x4<f32>,
    view: mat4x4<f32>,
    model: mat4x4<f32>,
    color: vec4<f32>,
    mode: vec4<u32>,
    palette: array<vec4<f32>, 16>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

const FLAT_FILL_PALETTE_PAIR: u32 = 1u;
const SOURCE_RASTER_HEIGHT: f32 = 224.0;
const SOURCE_RASTER_CENTER_X: f32 = 128.0;

struct FlatOut { @builtin(position) clip: vec4<f32> };

@vertex
fn vs_flat(@location(0) pos: vec3<f32>) -> FlatOut {
    var o: FlatOut;
    o.clip = u.proj * u.view * u.model * vec4<f32>(pos, 1.0);
    return o;
}

@fragment
fn fs_flat(in: FlatOut) -> @location(0) vec4<f32> {
    if (u.mode.x == FLAT_FILL_PALETTE_PAIR) {
        // Preserve source-raster parity under arbitrary output scaling. The
        // presentation rectangle is carried in color as x, y, width, height.
        let source_scale = SOURCE_RASTER_HEIGHT / u.color.w;
        let source_x = i32(floor(
            (in.clip.x - u.color.x - u.color.z * 0.5) * source_scale
                + SOURCE_RASTER_CENTER_X,
        ));
        let source_y = i32(floor((in.clip.y - u.color.y) * source_scale));
        var palette_index = u.mode.y;
        if (((source_x ^ source_y) & 1) != 0) {
            palette_index = u.mode.z;
        }
        // Source color zero is transparent before depth is written.
        if (palette_index == 0u) { discard; }
        return u.palette[min(palette_index, 15u)];
    }
    return u.color;
}

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct Tex3Out {
    @builtin(position) clip: vec4<f32>,
    // The Super FX scan converter advances texture coordinates linearly in
    // screen space.  WGSL `linear` interpolation is the non-perspective mode.
    @location(0) @interpolate(linear) tex_info: vec4<f32>,
};

@vertex
fn vs_textured(
    @location(0) pos: vec3<f32>,
    @location(1) tex_info: vec4<f32>,
) -> Tex3Out {
    var o: Tex3Out;
    o.clip = u.proj * u.view * u.model * vec4<f32>(pos, 1.0);
    o.tex_info = tex_info;
    return o;
}

@fragment
fn fs_textured(in: Tex3Out) -> @location(0) vec4<f32> {
    // MDRAWP.MC: `merge; and rmask; add rspdata; getc`.  `merge` takes the
    // integer bytes of the 8.8 coordinates, the 16-bit mask wraps X/Y, and
    // the result is then added to the linear sprite base address.  Performing
    // this after interpolation is important: adding atlas UVs at vertices
    // loses carries across 256-byte rows.
    let mask = u32(in.tex_info.w);
    let local_x = u32(i32(floor(in.tex_info.x)) & i32(mask & 0xffu));
    let local_y = u32(i32(floor(in.tex_info.y)) & i32((mask >> 8u) & 0xffu));
    let address = (u32(in.tex_info.z) + (local_y << 8u) + local_x) & 0x7fffu;
    let texel = textureLoad(
        tex,
        vec2<i32>(i32(address & 0xffu), i32(address >> 8u)),
        0,
    ).r;
    let packed = i32(texel * 255.0 + 0.5);
    var idx = packed % 16;
    if (u.mode.x == 3u) { idx = packed / 16; }
    if (idx == 0) { discard; }
    return u.palette[clamp(idx, 0, 15)];
}

struct OvOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_overlay(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>) -> OvOut {
    var o: OvOut;
    o.clip = u.proj * u.model * vec4<f32>(pos, 0.0, 1.0);
    o.uv = uv;
    return o;
}

@fragment
fn fs_overlay(in: OvOut) -> @location(0) vec4<f32> {
    let m = u.mode.x;
    if (m == 2u) {
        let idx_f = textureSample(tex, samp, in.uv).r;
        let idx = i32(idx_f * 255.0 + 0.5);
        if (idx == 0) { discard; }
        return u.palette[clamp(idx, 0, 15)];
    } else if (m == 1u) {
        let texel = textureSample(tex, samp, in.uv);
        if (texel.a < 0.5) { discard; }
        return texel * u.color;
    }
    return u.color;
}

struct DisplayBrightnessOut {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn vs_display_brightness(@builtin(vertex_index) vertex_index: u32) -> DisplayBrightnessOut {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: DisplayBrightnessOut;
    out.clip = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return out;
}

@fragment
fn fs_display_brightness(in: DisplayBrightnessOut) -> @location(0) vec4<f32> {
    let texel = textureLoad(tex, vec2<i32>(in.clip.xy), 0);
    let source = vec3<u32>(texel.rgb * 255.0 + vec3<f32>(0.5)) >> vec3<u32>(3u);
    let subtracted = source - min(source, vec3<u32>(u.mode.y));
    let scaled = subtracted * u.mode.x / 15u;
    let expanded = (scaled << vec3<u32>(3u)) | (scaled >> vec3<u32>(2u));
    return vec4<f32>(vec3<f32>(expanded) / 255.0, texel.a);
}
"#;
