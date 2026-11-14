use crate::atlas::TextureAtlas;
use crate::font::{self, FONT_TEX_H, FONT_TEX_W};
use crate::mesher::{face_verts, ChunkMeshResult, MeshData, WorldVertex};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use mc_core::{BlockPos, ChunkPos, DimensionId, Direction, SharedConstants};
use mc_data::{GameData, ItemId};
use mc_entities::{EntityManager, Player};
use mc_world::World;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct WorldUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub fog_color: [f32; 4],
    pub fog_params: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct UiUniforms {
    pub screen_size: [f32; 2],
    pub _pad: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl UiVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Immediate-mode 2D draw list accumulated by UI screens each frame.
#[derive(Default)]
pub struct UiBatch {
    pub atlas_verts: Vec<UiVertex>,
    pub atlas_indices: Vec<u32>,
    pub font_verts: Vec<UiVertex>,
    pub font_indices: Vec<u32>,
    pub white_uv: [f32; 4],
}

impl UiBatch {
    pub fn clear(&mut self, white_uv: [f32; 4]) {
        self.atlas_verts.clear();
        self.atlas_indices.clear();
        self.font_verts.clear();
        self.font_indices.clear();
        self.white_uv = white_uv;
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        Self::push_quad(
            &mut self.atlas_verts,
            &mut self.atlas_indices,
            x,
            y,
            w,
            h,
            self.white_uv,
            color,
        );
    }

    pub fn rect_outline(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, color: [f32; 4]) {
        self.rect(x, y, w, t, color);
        self.rect(x, y + h - t, w, t, color);
        self.rect(x, y, t, h, color);
        self.rect(x + w - t, y, t, h, color);
    }

    pub fn textured_rect(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], color: [f32; 4]) {
        Self::push_quad(
            &mut self.atlas_verts,
            &mut self.atlas_indices,
            x,
            y,
            w,
            h,
            uv,
            color,
        );
    }

    pub fn text(&mut self, x: f32, y: f32, scale: f32, text: &str, color: [f32; 4]) {
        let adv = 7.0 * scale;
        let cw = 8.0 * scale;
        let ch = 8.0 * scale;
        let mut cx = x;
        for c in text.chars() {
            if c == '\n' {
                continue;
            }
            if c != ' ' {
                let uv = font::char_uv(c);
                Self::push_quad(
                    &mut self.font_verts,
                    &mut self.font_indices,
                    cx,
                    y,
                    cw,
                    ch,
                    uv,
                    color,
                );
            }
            cx += adv;
        }
    }

    pub fn text_shadow(&mut self, x: f32, y: f32, scale: f32, text: &str, color: [f32; 4]) {
        let shadow = [color[0] * 0.25, color[1] * 0.25, color[2] * 0.25, color[3]];
        self.text(x + scale, y + scale, scale, text, shadow);
        self.text(x, y, scale, text, color);
    }

    pub fn text_width(scale: f32, text: &str) -> f32 {
        text.chars().count() as f32 * 7.0 * scale
    }

    fn push_quad(
        verts: &mut Vec<UiVertex>,
        indices: &mut Vec<u32>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv: [f32; 4],
        color: [f32; 4],
    ) {
        let b = verts.len() as u32;
        verts.push(UiVertex { pos: [x, y], uv: [uv[0], uv[1]], color });
        verts.push(UiVertex { pos: [x, y + h], uv: [uv[0], uv[3]], color });
        verts.push(UiVertex { pos: [x + w, y + h], uv: [uv[2], uv[3]], color });
        verts.push(UiVertex { pos: [x + w, y], uv: [uv[2], uv[1]], color });
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
}

struct GpuMeshSlice {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    index_count: u32,
}

struct GpuChunk {
    opaque: Option<GpuMeshSlice>,
    cutout: Option<GpuMeshSlice>,
    translucent: Option<GpuMeshSlice>,
}

pub struct FrameRenderInput<'a> {
    pub world: Option<&'a World>,
    pub player: Option<&'a Player>,
    pub entities: Option<&'a EntityManager>,
    pub data: &'a GameData,
    pub target_block: Option<BlockPos>,
    pub break_progress: f32,
    pub third_person: bool,
    pub fov_deg: f32,
    pub render_distance: i32,
    pub time_secs: f32,
    pub swing_progress: f32,
}

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    depth_view: wgpu::TextureView,

    world_uniform_buf: wgpu::Buffer,
    world_uniform_bg: wgpu::BindGroup,
    sky_uniform_buf: wgpu::Buffer,
    sky_uniform_bg: wgpu::BindGroup,
    cutout_uniform_buf: wgpu::Buffer,
    cutout_uniform_bg: wgpu::BindGroup,

    atlas_bg: wgpu::BindGroup,
    font_bg: wgpu::BindGroup,
    ui_uniform_buf: wgpu::Buffer,
    ui_uniform_bg: wgpu::BindGroup,

    opaque_pipeline: wgpu::RenderPipeline,
    cutout_pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,

    pub atlas: TextureAtlas,
    chunks: HashMap<ChunkPos, GpuChunk>,
}

impl Renderer {
    pub fn new(window: Arc<Window>, data: &GameData) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(window).expect("create wgpu surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("find suitable GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("minecraft_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .expect("create wgpu device");

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_view = create_depth_view(&device, config.width, config.height);
        let atlas = TextureAtlas::build(data);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pixel_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tex_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        let atlas_bg = upload_rgba_texture(&device, &queue, &tex_bgl, &sampler, "atlas", atlas.width, atlas.height, &atlas.rgba);
        let font_rgba = font::build_font_rgba();
        let font_bg = upload_rgba_texture(&device, &queue, &tex_bgl, &sampler, "font", FONT_TEX_W, FONT_TEX_H, &font_rgba);

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let (world_uniform_buf, world_uniform_bg) = create_uniform_bg(&device, &uniform_bgl, "world_u", &WorldUniforms::zeroed());
        let (cutout_uniform_buf, cutout_uniform_bg) = create_uniform_bg(&device, &uniform_bgl, "cutout_u", &WorldUniforms::zeroed());
        let (sky_uniform_buf, sky_uniform_bg) = create_uniform_bg(&device, &uniform_bgl, "sky_u", &WorldUniforms::zeroed());
        let (ui_uniform_buf, ui_uniform_bg) = create_uniform_bg(
            &device,
            &uniform_bgl,
            "ui_u",
            &UiUniforms {
                screen_size: [config.width as f32, config.height as f32],
                _pad: [0.0; 2],
            },
        );

        let world_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let world_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world_pl_layout"),
            bind_group_layouts: &[&uniform_bgl, &tex_bgl],
            push_constant_ranges: &[],
        });

        let opaque_pipeline = build_world_pipeline(&device, &world_pl_layout, &world_shader, surface_format, true, Some(wgpu::Face::Back), false, "opaque");
        let cutout_pipeline = build_world_pipeline(&device, &world_pl_layout, &world_shader, surface_format, true, None, false, "cutout");
        let translucent_pipeline = build_world_pipeline(&device, &world_pl_layout, &world_shader, surface_format, true, None, true, "translucent");
        let sky_pipeline = build_world_pipeline(&device, &world_pl_layout, &world_shader, surface_format, false, None, true, "sky");

        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui_shader.wgsl").into()),
        });
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui_pipeline"),
            layout: Some(&world_pl_layout),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs_ui"),
                buffers: &[UiVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_ui"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            depth_view,
            world_uniform_buf,
            world_uniform_bg,
            sky_uniform_buf,
            sky_uniform_bg,
            cutout_uniform_buf,
            cutout_uniform_bg,
            atlas_bg,
            font_bg,
            ui_uniform_buf,
            ui_uniform_bg,
            opaque_pipeline,
            cutout_pipeline,
            translucent_pipeline,
            sky_pipeline,
            ui_pipeline,
            atlas,
            chunks: HashMap::new(),
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_view(&self.device, new_size.width, new_size.height);
        self.queue.write_buffer(
            &self.ui_uniform_buf,
            0,
            bytemuck::bytes_of(&UiUniforms {
                screen_size: [new_size.width as f32, new_size.height as f32],
                _pad: [0.0; 2],
            }),
        );
    }

    pub fn clear_chunks(&mut self) {
        self.chunks.clear();
    }

    pub fn upload_chunk_mesh(&mut self, mesh: ChunkMeshResult) {
        let to_gpu = |m: MeshData| -> Option<GpuMeshSlice> {
            if m.indices.is_empty() {
                return None;
            }
            let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk_vbuf"),
                contents: bytemuck::cast_slice(&m.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk_ibuf"),
                contents: bytemuck::cast_slice(&m.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            Some(GpuMeshSlice {
                vbuf,
                ibuf,
                index_count: m.indices.len() as u32,
            })
        };
        self.chunks.insert(
            mesh.chunk_pos,
            GpuChunk {
                opaque: to_gpu(mesh.opaque),
                cutout: to_gpu(mesh.cutout),
                translucent: to_gpu(mesh.translucent),
            },
        );
    }

    pub fn retain_chunks(&mut self, center: ChunkPos, radius: i32) {
        self.chunks
            .retain(|pos, _| (pos.x - center.x).abs() <= radius && (pos.z - center.z).abs() <= radius);
    }

    pub fn render(&mut self, input: FrameRenderInput<'_>, ui: &UiBatch) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame_encoder"),
        });

        let (sky_rgb, fog_rgb, sky_brightness, sun_angle) = compute_atmosphere(input.world, input.player);
        let aspect = self.config.width.max(1) as f32 / self.config.height.max(1) as f32;

        // Build dynamic 3D geometry (sky, entities, highlight, crack overlay, hand) before RenderPass
        let mut sky_mesh = MeshData::default();
        let mut entity_mesh = MeshData::default();
        let mut overlay_mesh = MeshData::default();

        let mut world_vp = Mat4::IDENTITY;
        let mut cam_pos = Vec3::ZERO;
        let fog_dist = (input.render_distance.max(2) * 16) as f32;

        if let (Some(world), Some(player)) = (input.world, input.player) {
            let eye = player.eye_pos();
            let look = player.look_dir();
            let eye_f = Vec3::new(eye.x as f32, eye.y as f32, eye.z as f32);
            let look_f = Vec3::new(look.x as f32, look.y as f32, look.z as f32).normalize_or(Vec3::Z);
            cam_pos = if input.third_person {
                eye_f - look_f * 4.2 + Vec3::Y * 0.4
            } else {
                eye_f
            };
            let target = cam_pos + look_f * 10.0;
            let view_mat = mat4_look_at_rh(cam_pos, target, Vec3::Y);
            let proj_mat = mat4_perspective_rh(input.fov_deg.to_radians(), aspect, 0.08, fog_dist * 2.2);
            world_vp = proj_mat * view_mat;

            let view_only = mat4_look_at_rh(Vec3::ZERO, look_f, Vec3::Y);
            let sky_vp = proj_mat * view_only;

            self.queue.write_buffer(
                &self.sky_uniform_buf,
                0,
                bytemuck::bytes_of(&WorldUniforms {
                    view_proj: sky_vp.to_cols_array_2d(),
                    camera_pos: [0.0, 0.0, 0.0, input.time_secs],
                    fog_color: [fog_rgb[0], fog_rgb[1], fog_rgb[2], sky_brightness],
                    fog_params: [1000.0, 2000.0, 0.0, 1.0],
                }),
            );
            self.queue.write_buffer(
                &self.world_uniform_buf,
                0,
                bytemuck::bytes_of(&WorldUniforms {
                    view_proj: world_vp.to_cols_array_2d(),
                    camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z, input.time_secs],
                    fog_color: [fog_rgb[0], fog_rgb[1], fog_rgb[2], sky_brightness],
                    fog_params: [fog_dist * 0.65, fog_dist * 1.05, 0.0, 0.0],
                }),
            );
            self.queue.write_buffer(
                &self.cutout_uniform_buf,
                0,
                bytemuck::bytes_of(&WorldUniforms {
                    view_proj: world_vp.to_cols_array_2d(),
                    camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z, input.time_secs],
                    fog_color: [fog_rgb[0], fog_rgb[1], fog_rgb[2], sky_brightness],
                    fog_params: [fog_dist * 0.65, fog_dist * 1.05, 0.5, 0.0],
                }),
            );

            build_sky_geometry(
                world.dimension,
                sun_angle,
                cam_pos,
                sky_brightness,
                input.time_secs,
                self.atlas.white_uv,
                &mut sky_mesh,
            );
            if let Some(entities) = input.entities {
                build_entities_and_particles(entities, &self.atlas, &mut entity_mesh, &mut overlay_mesh);
            }
            if input.third_person {
                build_player_model(player, &self.atlas, &mut entity_mesh);
            } else {
                build_first_person_hand(player, cam_pos, look_f, input.swing_progress, &self.atlas, &mut overlay_mesh);
            }
            if let Some(tpos) = input.target_block {
                build_block_target_and_cracks(
                    tpos,
                    input.break_progress,
                    &self.atlas,
                    &mut overlay_mesh,
                );
            }
        }

        let sky_gpu = upload_mesh(&self.device, "sky", &sky_mesh);
        let entity_gpu = upload_mesh(&self.device, "entity", &entity_mesh);
        let overlay_gpu = upload_mesh(&self.device, "overlay", &overlay_mesh);

        let ui_atlas_gpu = upload_ui_mesh(&self.device, "ui_atlas", &ui.atlas_verts, &ui.atlas_indices);
        let ui_font_gpu = upload_ui_mesh(&self.device, "ui_font", &ui.font_verts, &ui.font_indices);

        if input.world.is_some() {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: sky_rgb[0] as f64,
                            g: sky_rgb[1] as f64,
                            b: sky_rgb[2] as f64,
                            a: 1.0,
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

            if let Some(ref g) = sky_gpu {
                rpass.set_pipeline(&self.sky_pipeline);
                rpass.set_bind_group(0, &self.sky_uniform_bg, &[]);
                rpass.set_bind_group(1, &self.atlas_bg, &[]);
                draw_slice(&mut rpass, g);
            }

            // Opaque terrain
            rpass.set_pipeline(&self.opaque_pipeline);
            rpass.set_bind_group(0, &self.world_uniform_bg, &[]);
            rpass.set_bind_group(1, &self.atlas_bg, &[]);
            for chunk in self.chunks.values() {
                if let Some(ref g) = chunk.opaque {
                    draw_slice(&mut rpass, g);
                }
            }
            if let Some(ref g) = entity_gpu {
                draw_slice(&mut rpass, g);
            }

            // Cutout terrain (leaves, grass, flowers, torches, overlay)
            rpass.set_pipeline(&self.cutout_pipeline);
            rpass.set_bind_group(0, &self.cutout_uniform_bg, &[]);
            rpass.set_bind_group(1, &self.atlas_bg, &[]);
            for chunk in self.chunks.values() {
                if let Some(ref g) = chunk.cutout {
                    draw_slice(&mut rpass, g);
                }
            }

            // Translucent pass (water, ice, portals, block target & cracks)
            rpass.set_pipeline(&self.translucent_pipeline);
            rpass.set_bind_group(0, &self.world_uniform_bg, &[]);
            rpass.set_bind_group(1, &self.atlas_bg, &[]);
            for chunk in self.chunks.values() {
                if let Some(ref g) = chunk.translucent {
                    draw_slice(&mut rpass, g);
                }
            }
            if let Some(ref g) = overlay_gpu {
                rpass.set_bind_group(0, &self.world_uniform_bg, &[]);
                draw_slice(&mut rpass, g);
            }
        }

        // Separate 2D UI & Bitmap Font pass (no depth attachment, compatible with ui_pipeline)
        {
            let color_load = if input.world.is_some() {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(wgpu::Color {
                    r: sky_rgb[0] as f64,
                    g: sky_rgb[1] as f64,
                    b: sky_rgb[2] as f64,
                    a: 1.0,
                })
            };
            let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: color_load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            ui_pass.set_pipeline(&self.ui_pipeline);
            ui_pass.set_bind_group(0, &self.ui_uniform_bg, &[]);
            if let Some(ref g) = ui_atlas_gpu {
                ui_pass.set_bind_group(1, &self.atlas_bg, &[]);
                draw_slice(&mut ui_pass, g);
            }
            if let Some(ref g) = ui_font_gpu {
                ui_pass.set_bind_group(1, &self.font_bg, &[]);
                draw_slice(&mut ui_pass, g);
            }
        }

        let _ = (world_vp, cam_pos);
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn item_icon_uv(&self, item: ItemId) -> [f32; 4] {
        self.atlas.item_uv(item)
    }
}

fn draw_slice<'a>(rpass: &mut wgpu::RenderPass<'a>, s: &'a GpuMeshSlice) {
    rpass.set_vertex_buffer(0, s.vbuf.slice(..));
    rpass.set_index_buffer(s.ibuf.slice(..), wgpu::IndexFormat::Uint32);
    rpass.draw_indexed(0..s.index_count, 0, 0..1);
}

fn upload_mesh(device: &wgpu::Device, label: &str, m: &MeshData) -> Option<GpuMeshSlice> {
    if m.indices.is_empty() {
        return None;
    }
    Some(GpuMeshSlice {
        vbuf: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&m.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }),
        ibuf: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&m.indices),
            usage: wgpu::BufferUsages::INDEX,
        }),
        index_count: m.indices.len() as u32,
    })
}

fn upload_ui_mesh(device: &wgpu::Device, label: &str, v: &[UiVertex], i: &[u32]) -> Option<GpuMeshSlice> {
    if i.is_empty() {
        return None;
    }
    Some(GpuMeshSlice {
        vbuf: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(v),
            usage: wgpu::BufferUsages::VERTEX,
        }),
        ibuf: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(i),
            usage: wgpu::BufferUsages::INDEX,
        }),
        index_count: i.len() as u32,
    })
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth_tex"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn upload_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::BindGroup {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
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
    })
}

fn create_uniform_bg<T: Pod>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
    initial: &T,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::bytes_of(initial),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buf.as_entire_binding(),
        }],
    });
    (buf, bg)
}

fn build_world_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    depth_write: bool,
    cull_mode: Option<wgpu::Face>,
    alpha_blend: bool,
    label: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[WorldVertex::desc()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: if alpha_blend {
                    Some(wgpu::BlendState::ALPHA_BLENDING)
                } else {
                    Some(wgpu::BlendState::REPLACE)
                },
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: depth_write,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn mat4_look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4 {
    let f = (center - eye).normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    Mat4::from_cols_array(&[
        s.x, u.x, -f.x, 0.0,
        s.y, u.y, -f.y, 0.0,
        s.z, u.z, -f.z, 0.0,
        -s.dot(eye), -u.dot(eye), f.dot(eye), 1.0,
    ])
}

fn mat4_perspective_rh(fov_y_rad: f32, aspect: f32, z_near: f32, z_far: f32) -> Mat4 {
    let f = 1.0 / (fov_y_rad * 0.5).tan();
    let nf = 1.0 / (z_near - z_far);
    Mat4::from_cols_array(&[
        f / aspect, 0.0, 0.0, 0.0,
        0.0, f, 0.0, 0.0,
        0.0, 0.0, z_far * nf, -1.0,
        0.0, 0.0, z_near * z_far * nf, 0.0,
    ])
}

fn compute_atmosphere(world: Option<&World>, player: Option<&Player>) -> ([f32; 3], [f32; 3], f32, f32) {
    let Some(w) = world else {
        return ([0.10, 0.14, 0.22], [0.10, 0.14, 0.22], 1.0, 0.0);
    };
    if let Some(p) = player {
        if p.is_in_water {
            return ([0.05, 0.18, 0.42], [0.05, 0.20, 0.48], 0.8, 0.0);
        }
        if p.is_in_lava {
            return ([0.62, 0.16, 0.04], [0.68, 0.18, 0.04], 0.8, 0.0);
        }
    }
    match w.dimension {
        DimensionId::TheNether => ([0.20, 0.03, 0.03], [0.22, 0.04, 0.04], 0.85, 0.0),
        DimensionId::TheEnd => ([0.05, 0.03, 0.09], [0.07, 0.04, 0.12], 0.88, 0.0),
        DimensionId::Overworld => {
            let frac = (w.day_time.rem_euclid(SharedConstants::DAY_TICKS) as f32)
                / (SharedConstants::DAY_TICKS as f32);
            let angle = frac * std::f32::consts::TAU;
            // Peak day around tick 6000 (frac = 0.25 => sin(angle) = 1.0)
            let day = (angle.sin() * 1.4 + 0.35).clamp(0.0, 1.0);
            let rain_dim = if w.is_raining { 0.65 } else { 1.0 };
            let brightness = (0.14 + 0.86 * day) * rain_dim;
            let sky = [
                (0.03 + 0.44 * day) * rain_dim,
                (0.04 + 0.62 * day) * rain_dim,
                (0.09 + 0.91 * day) * rain_dim,
            ];
            let fog = [
                (0.04 + 0.71 * day) * rain_dim,
                (0.05 + 0.80 * day) * rain_dim,
                (0.10 + 0.90 * day) * rain_dim,
            ];
            (sky, fog, brightness, angle)
        }
    }
}

fn build_sky_geometry(
    dim: DimensionId,
    sun_angle: f32,
    _cam_pos: Vec3,
    sky_brightness: f32,
    time_secs: f32,
    uv: [f32; 4],
    mesh: &mut MeshData,
) {
    if dim != DimensionId::Overworld {
        return;
    }
    let d = 90.0f32;
    let sun_dir = Vec3::new(sun_angle.cos(), sun_angle.sin(), 0.25).normalize();
    let emit_celestial = |dir: Vec3, sz: f32, col: [f32; 4], mesh: &mut MeshData| {
        let center = dir * d;
        let up = if dir.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
        let right = dir.cross(up).normalize() * sz;
        let uvec = right.cross(dir).normalize() * sz;
        let p0 = (center - right + uvec).to_array();
        let p1 = (center - right - uvec).to_array();
        let p2 = (center + right - uvec).to_array();
        let p3 = (center + right + uvec).to_array();
        mesh.push_quad(p0, p1, p2, p3, uv, [col; 4], [[1.0, 1.0]; 4]);
    };
    emit_celestial(sun_dir, 9.5, [1.0, 0.97, 0.78, 1.0], mesh);
    emit_celestial(-sun_dir, 6.5, [0.88, 0.92, 1.0, 0.95], mesh);

    // Stars visible at night
    let night_alpha = ((0.35 - sky_brightness) * 3.0).clamp(0.0, 0.95);
    if night_alpha > 0.05 {
        for i in 0..90u32 {
            let h1 = ((i.wrapping_mul(2654435761)) & 0xFFFF) as f32 / 65535.0;
            let h2 = ((i.wrapping_mul(1597334677)) & 0xFFFF) as f32 / 65535.0;
            let theta = h1 * std::f32::consts::TAU;
            let phi = 0.15 + h2 * 1.25;
            let sdir = Vec3::new(phi.cos() * theta.cos(), phi.sin(), phi.cos() * theta.sin());
            emit_celestial(sdir, 0.45, [0.95, 0.97, 1.0, night_alpha], mesh);
        }
    }

    // Cloud band near Overworld cloud_height = 192.33 (relative to camera height)
    let cloud_y = 64.0f32;
    let drift = (time_secs * 0.6) % 160.0;
    let c_col = [
        0.92 * (0.25 + 0.75 * sky_brightness),
        0.94 * (0.25 + 0.75 * sky_brightness),
        0.98 * (0.30 + 0.70 * sky_brightness),
        0.72,
    ];
    for ix in -4..=4 {
        for iz in -4..=4 {
            if ((ix * 7 + iz * 13) & 3) != 0 {
                continue;
            }
            let cx = ix as f32 * 22.0 - drift + 80.0;
            let cz = iz as f32 * 22.0;
            let min = [cx - 8.0, cloud_y, cz - 8.0];
            let max = [cx + 8.0, cloud_y + 2.5, cz + 8.0];
            for dir in Direction::ALL {
                let [p0, p1, p2, p3] = face_verts(min, max, dir);
                mesh.push_quad(p0, p1, p2, p3, uv, [c_col; 4], [[1.0, 1.0]; 4]);
            }
        }
    }
}

fn push_colored_box(min: [f32; 3], max: [f32; 3], uv: [f32; 4], color: [f32; 4], mesh: &mut MeshData) {
    for dir in Direction::ALL {
        let shade = match dir {
            Direction::Up => 1.0,
            Direction::Down => 0.55,
            Direction::North | Direction::South => 0.85,
            Direction::East | Direction::West => 0.72,
        };
        let c = [color[0] * shade, color[1] * shade, color[2] * shade, color[3]];
        let [p0, p1, p2, p3] = face_verts(min, max, dir);
        mesh.push_quad(p0, p1, p2, p3, uv, [c; 4], [[1.0, 0.85]; 4]);
    }
}

fn build_entities_and_particles(
    entities: &EntityManager,
    atlas: &TextureAtlas,
    mesh: &mut MeshData,
    overlay_mesh: &mut MeshData,
) {
    let wuv = atlas.white_uv;
    for ent in &entities.entities {
        if ent.removed {
            continue;
        }
        let (w, h) = ent.kind.dimensions();
        let hw = w as f32 * 0.5;
        let x = ent.pos.x as f32;
        let y = ent.pos.y as f32;
        let z = ent.pos.z as f32;
        let mut col = ent.kind.color_rgba();
        if ent.hurt_ticks > 0 {
            col = [1.0, 0.22, 0.22, 1.0];
        }
        if let Some(stack) = &ent.item_stack {
            let iuv = atlas.item_uv(stack.item);
            push_colored_box([x - 0.16, y + 0.08, z - 0.16], [x + 0.16, y + 0.40, z + 0.16], iuv, [1.0; 4], mesh);
        } else if let Some(fs) = ent.falling_state {
            let buv = atlas.block_uv(fs.id()).front;
            push_colored_box([x - 0.49, y, z - 0.49], [x + 0.49, y + 0.98, z + 0.49], buv, [1.0; 4], mesh);
        } else {
            // Body + head silhouette so mobs look unmistakably alive
            let body_h = h as f32 * 0.65;
            push_colored_box([x - hw, y, z - hw], [x + hw, y + body_h, z + hw], wuv, col, mesh);
            let head_hw = hw * 0.85;
            let head_col = [col[0] * 1.1, col[1] * 1.1, col[2] * 1.1, 1.0];
            push_colored_box(
                [x - head_hw, y + body_h, z - head_hw],
                [x + head_hw, y + h as f32, z + head_hw],
                wuv,
                head_col.map(|v| v.min(1.0)),
                mesh,
            );
        }
    }

    for p in &entities.particles {
        let s = p.size * 0.5;
        let x = p.pos.x as f32;
        let y = p.pos.y as f32;
        let z = p.pos.z as f32;
        push_colored_box([x - s, y - s, z - s], [x + s, y + s, z + s], wuv, p.color, overlay_mesh);
    }
}

fn build_player_model(player: &Player, atlas: &TextureAtlas, mesh: &mut MeshData) {
    let x = player.pos.x as f32;
    let y = player.pos.y as f32;
    let z = player.pos.z as f32;
    let wuv = atlas.white_uv;
    // Legs, cyan Steve tunic torso, peach head
    push_colored_box([x - 0.24, y, z - 0.14], [x + 0.24, y + 0.75, z + 0.14], wuv, [0.22, 0.24, 0.58, 1.0], mesh);
    push_colored_box([x - 0.26, y + 0.75, z - 0.15], [x + 0.26, y + 1.40, z + 0.15], wuv, [0.16, 0.68, 0.74, 1.0], mesh);
    push_colored_box([x - 0.22, y + 1.40, z - 0.22], [x + 0.22, y + 1.84, z + 0.22], wuv, [0.85, 0.66, 0.52, 1.0], mesh);
}

fn build_first_person_hand(
    player: &Player,
    cam_pos: Vec3,
    look: Vec3,
    swing_progress: f32,
    atlas: &TextureAtlas,
    mesh: &mut MeshData,
) {
    let right = look.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(look).normalize_or(Vec3::Y);
    let swing = (swing_progress * std::f32::consts::PI).sin();
    let anchor = cam_pos + look * (0.55 - swing * 0.18) + right * 0.34 - up * (0.28 + swing * 0.12);
    let stack = player.main_hand();
    if stack.is_empty() {
        let s = 0.08;
        push_colored_box(
            [anchor.x - s, anchor.y - s * 1.4, anchor.z - s],
            [anchor.x + s, anchor.y + s * 1.4, anchor.z + s],
            atlas.white_uv,
            [0.86, 0.67, 0.53, 1.0],
            mesh,
        );
    } else {
        let uv = atlas.item_uv(stack.item);
        let s = 0.11;
        push_colored_box(
            [anchor.x - s, anchor.y - s, anchor.z - s],
            [anchor.x + s, anchor.y + s, anchor.z + s],
            uv,
            [1.0, 1.0, 1.0, 1.0],
            mesh,
        );
    }
}

fn build_block_target_and_cracks(
    pos: BlockPos,
    break_progress: f32,
    atlas: &TextureAtlas,
    mesh: &mut MeshData,
) {
    let x0 = pos.x as f32 - 0.004;
    let y0 = pos.y as f32 - 0.004;
    let z0 = pos.z as f32 - 0.004;
    let x1 = pos.x as f32 + 1.004;
    let y1 = pos.y as f32 + 1.004;
    let z1 = pos.z as f32 + 1.004;
    let wuv = atlas.white_uv;
    let bcol = [0.05, 0.05, 0.05, 0.55];
    let t = 0.015;
    // 12 crisp wireframe edges around the targeted block
    let edges = [
        ([x0, y0, z0], [x1, y0 + t, z0 + t]),
        ([x0, y1 - t, z0], [x1, y1, z0 + t]),
        ([x0, y0, z1 - t], [x1, y0 + t, z1]),
        ([x0, y1 - t, z1 - t], [x1, y1, z1]),
        ([x0, y0, z0], [x0 + t, y1, z0 + t]),
        ([x1 - t, y0, z0], [x1, y1, z0 + t]),
        ([x0, y0, z1 - t], [x0 + t, y1, z1]),
        ([x1 - t, y0, z1 - t], [x1, y1, z1]),
        ([x0, y0, z0], [x0 + t, y0 + t, z1]),
        ([x1 - t, y0, z0], [x1, y0 + t, z1]),
        ([x0, y1 - t, z0], [x0 + t, y1, z1]),
        ([x1 - t, y1 - t, z0], [x1, y1, z1]),
    ];
    for (mn, mx) in edges {
        push_colored_box(mn, mx, wuv, bcol, mesh);
    }

    if break_progress > 0.01 {
        let stage = ((break_progress * 9.99) as usize).min(9);
        let cuv = atlas.destroy_uvs[stage];
        let min = [pos.x as f32 - 0.003, pos.y as f32 - 0.003, pos.z as f32 - 0.003];
        let max = [pos.x as f32 + 1.003, pos.y as f32 + 1.003, pos.z as f32 + 1.003];
        for dir in Direction::ALL {
            let [p0, p1, p2, p3] = face_verts(min, max, dir);
            mesh.push_quad(p0, p1, p2, p3, cuv, [[1.0, 1.0, 1.0, 0.85]; 4], [[1.0, 1.0]; 4]);
        }
    }
}
