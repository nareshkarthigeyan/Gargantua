//! wgpu render graph: lensed HDR scene, spacecraft, additive trail, bloom and
//! the final display transform.

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::ship_mesh::ShipVertex;

pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SceneUniform {
    pub view_proj: [[f32; 4]; 4],
    pub cam_pos_time: [f32; 4],
    pub cam_right_fov: [f32; 4],
    pub cam_up_aspect: [f32; 4],
    pub cam_forward_disk: [f32; 4],
    pub viewport_disk: [f32; 4],
    pub ship_pos: [f32; 4],
    pub misc: [f32; 4],
    pub cam_velocity: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PostUniform {
    pub params: [f32; 4], // bloom intensity, exposure, time, grain
    pub screen: [f32; 4], // width, height, danger, aspect
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BloomParams {
    values: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TrailVertex {
    pub position: [f32; 3],
    pub _pad: f32,
    pub color: [f32; 4],
}

pub struct FrameData<'a> {
    pub scene: SceneUniform,
    pub post: PostUniform,
    pub ship_vertices: &'a [ShipVertex],
    pub trail_vertices: &'a [TrailVertex],
}

pub struct Renderer {
    pub surface_size: (u32, u32),
    pub scene_size: (u32, u32),
    pub render_scale: f32,

    bloom_bgl: wgpu::BindGroupLayout,
    post_bgl: wgpu::BindGroupLayout,

    scene_uniform: wgpu::Buffer,
    scene_bg: wgpu::BindGroup,
    post_uniform: wgpu::Buffer,
    post_bg: wgpu::BindGroup,

    bright_params: wgpu::Buffer,
    blur_h_params: wgpu::Buffer,
    blur_v_params: wgpu::Buffer,
    bright_bg: wgpu::BindGroup,
    blur_h_bg: wgpu::BindGroup,
    blur_v_bg: wgpu::BindGroup,

    sampler: wgpu::Sampler,
    hdr_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    bloom_a_view: wgpu::TextureView,
    bloom_b_view: wgpu::TextureView,
    _hdr_texture: wgpu::Texture,
    _depth_texture: wgpu::Texture,
    _bloom_a: wgpu::Texture,
    _bloom_b: wgpu::Texture,

    bg_pipeline: wgpu::RenderPipeline,
    rocket_pipeline: wgpu::RenderPipeline,
    trail_pipeline: wgpu::RenderPipeline,
    bright_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,

    ship_buffer: wgpu::Buffer,
    ship_buffer_bytes: u64,
    trail_buffer: wgpu::Buffer,
    trail_buffer_bytes: u64,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        render_scale: f32,
    ) -> Self {
        let scene_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene bgl"),
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

        let bloom_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom bgl"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let post_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post bgl"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let scene_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene uniform"),
            contents: bytemuck::bytes_of(&SceneUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene bg"),
            layout: &scene_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scene_uniform.as_entire_binding(),
            }],
        });

        let post_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("post uniform"),
            contents: bytemuck::bytes_of(&PostUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bright_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bright params"),
            contents: bytemuck::bytes_of(&BloomParams {
                values: [0.85, 0.0, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let blur_h_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blur h params"),
            contents: bytemuck::bytes_of(&BloomParams { values: [0.0; 4] }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let blur_v_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blur v params"),
            contents: bytemuck::bytes_of(&BloomParams { values: [0.0; 4] }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bg_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("black hole shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blackhole.wgsl").into()),
        });
        let rocket_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rocket shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rocket.wgsl").into()),
        });
        let trail_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("trail shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/trail.wgsl").into()),
        });
        let bloom_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/bloom.wgsl").into()),
        });
        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/composite.wgsl").into()),
        });

        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene layout"),
            bind_group_layouts: &[&scene_bgl],
            push_constant_ranges: &[],
        });
        let bloom_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom layout"),
            bind_group_layouts: &[&bloom_bgl],
            push_constant_ranges: &[],
        });
        let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post layout"),
            bind_group_layouts: &[&post_bgl],
            push_constant_ranges: &[],
        });

        let depth_state = |write: bool, compare: wgpu::CompareFunction| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: write,
            depth_compare: compare,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };

        let bg_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("background pipeline"),
            layout: Some(&scene_layout),
            vertex: wgpu::VertexState {
                module: &bg_shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state(false, wgpu::CompareFunction::Always)),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &bg_shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        let ship_attributes = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x3,
            3 => Float32
        ];
        let rocket_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rocket pipeline"),
            layout: Some(&scene_layout),
            vertex: wgpu::VertexState {
                module: &rocket_shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<ShipVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ship_attributes,
                }],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(true, wgpu::CompareFunction::LessEqual)),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &rocket_shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        let trail_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let trail_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("trail pipeline"),
            layout: Some(&scene_layout),
            vertex: wgpu::VertexState {
                module: &trail_shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<TrailVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &trail_attributes,
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state(false, wgpu::CompareFunction::LessEqual)),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &trail_shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        let bright_pipeline = post_pipeline(
            device,
            "bright pipeline",
            &bloom_shader,
            "bright_fs",
            &bloom_layout,
            HDR_FORMAT,
            None,
        );
        let blur_pipeline = post_pipeline(
            device,
            "blur pipeline",
            &bloom_shader,
            "blur_fs",
            &bloom_layout,
            HDR_FORMAT,
            None,
        );
        let composite_pipeline = post_pipeline(
            device,
            "composite pipeline",
            &composite_shader,
            "fs_main",
            &post_layout,
            target_format,
            None,
        );

        let ship_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ship vertices"),
            size: 4 * 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let trail_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("trail vertices"),
            size: 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Build with a dummy 1x1 target first; resize() creates the real ones.
        let hdr_texture = create_texture(
            device,
            1,
            1,
            HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth_texture = create_texture(
            device,
            1,
            1,
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let bloom_a = create_texture(
            device,
            1,
            1,
            HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let bloom_b = create_texture(
            device,
            1,
            1,
            HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let hdr_view = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_a_view = bloom_a.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_b_view = bloom_b.create_view(&wgpu::TextureViewDescriptor::default());

        let bright_bg = create_bloom_bg(
            device,
            &bloom_bgl,
            &hdr_view,
            &sampler,
            &bright_params,
            "bright bg",
        );
        let blur_h_bg = create_bloom_bg(
            device,
            &bloom_bgl,
            &bloom_a_view,
            &sampler,
            &blur_h_params,
            "blur h bg",
        );
        let blur_v_bg = create_bloom_bg(
            device,
            &bloom_bgl,
            &bloom_b_view,
            &sampler,
            &blur_v_params,
            "blur v bg",
        );
        let post_bg = create_post_bg(
            device,
            &post_bgl,
            &hdr_view,
            &sampler,
            &bloom_a_view,
            &post_uniform,
        );

        let mut renderer = Self {
            surface_size: (1, 1),
            scene_size: (1, 1),
            render_scale,
            bloom_bgl,
            post_bgl,
            scene_uniform,
            scene_bg,
            post_uniform,
            post_bg,
            bright_params,
            blur_h_params,
            blur_v_params,
            bright_bg,
            blur_h_bg,
            blur_v_bg,
            sampler,
            hdr_view,
            depth_view,
            bloom_a_view,
            bloom_b_view,
            _hdr_texture: hdr_texture,
            _depth_texture: depth_texture,
            _bloom_a: bloom_a,
            _bloom_b: bloom_b,
            bg_pipeline,
            rocket_pipeline,
            trail_pipeline,
            bright_pipeline,
            blur_pipeline,
            composite_pipeline,
            ship_buffer,
            ship_buffer_bytes: 4 * 1024 * 1024,
            trail_buffer,
            trail_buffer_bytes: 1024 * 1024,
        };
        renderer.resize(device, queue, width, height);
        renderer
    }

    pub fn resize(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        let sw = ((width as f32 * self.render_scale).round() as u32).clamp(1, 4096);
        let sh = ((height as f32 * self.render_scale).round() as u32).clamp(1, 4096);
        if self.surface_size == (width, height) && self.scene_size == (sw, sh) {
            return;
        }
        self.surface_size = (width, height);
        self.scene_size = (sw, sh);

        let hdr = create_texture(
            device,
            sw,
            sh,
            HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = create_texture(
            device,
            sw,
            sh,
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let bw = (sw / 2).max(1);
        let bh = (sh / 2).max(1);
        let bloom_a = create_texture(
            device,
            bw,
            bh,
            HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let bloom_b = create_texture(
            device,
            bw,
            bh,
            HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let hdr_view = hdr.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_a_view = bloom_a.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_b_view = bloom_b.create_view(&wgpu::TextureViewDescriptor::default());

        self.bright_bg = create_bloom_bg(
            device,
            &self.bloom_bgl,
            &hdr_view,
            &self.sampler,
            &self.bright_params,
            "bright bg",
        );
        self.blur_h_bg = create_bloom_bg(
            device,
            &self.bloom_bgl,
            &bloom_a_view,
            &self.sampler,
            &self.blur_h_params,
            "blur h bg",
        );
        self.blur_v_bg = create_bloom_bg(
            device,
            &self.bloom_bgl,
            &bloom_b_view,
            &self.sampler,
            &self.blur_v_params,
            "blur v bg",
        );
        self.post_bg = create_post_bg(
            device,
            &self.post_bgl,
            &hdr_view,
            &self.sampler,
            &bloom_a_view,
            &self.post_uniform,
        );

        queue.write_buffer(
            &self.blur_h_params,
            0,
            bytemuck::bytes_of(&BloomParams {
                values: [0.0, 0.0, 3.2 / bw as f32, 0.0],
            }),
        );
        queue.write_buffer(
            &self.blur_v_params,
            0,
            bytemuck::bytes_of(&BloomParams {
                values: [0.0, 0.0, 0.0, 3.2 / bh as f32],
            }),
        );

        self._hdr_texture = hdr;
        self._depth_texture = depth;
        self._bloom_a = bloom_a;
        self._bloom_b = bloom_b;
        self.hdr_view = hdr_view;
        self.depth_view = depth_view;
        self.bloom_a_view = bloom_a_view;
        self.bloom_b_view = bloom_b_view;
    }

    pub fn set_render_scale(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scale: f32) {
        self.render_scale = scale.clamp(0.35, 1.0);
        let (w, h) = self.surface_size;
        self.surface_size = (0, 0);
        self.resize(device, queue, w, h);
    }

    pub fn encode_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        frame: &FrameData<'_>,
    ) {
        queue.write_buffer(&self.scene_uniform, 0, bytemuck::bytes_of(&frame.scene));
        queue.write_buffer(&self.post_uniform, 0, bytemuck::bytes_of(&frame.post));
        // Blur footprint follows angular disk size, in viewport units.
        let radius = 0.012 * frame.post.params[3];
        queue.write_buffer(
            &self.blur_h_params,
            0,
            bytemuck::bytes_of(&BloomParams {
                values: [0.0, 0.0, radius / frame.post.screen[3], 0.0],
            }),
        );
        queue.write_buffer(
            &self.blur_v_params,
            0,
            bytemuck::bytes_of(&BloomParams {
                values: [0.0, 0.0, 0.0, radius],
            }),
        );

        let ship_bytes = bytemuck::cast_slice(frame.ship_vertices);
        if !ship_bytes.is_empty() {
            let needed = ship_bytes.len() as u64;
            if needed > self.ship_buffer_bytes {
                let new_size = needed.next_power_of_two().max(self.ship_buffer_bytes * 2);
                self.ship_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ship vertices (grown)"),
                    size: new_size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.ship_buffer_bytes = new_size;
            }
            queue.write_buffer(&self.ship_buffer, 0, ship_bytes);
        }

        let trail_bytes = bytemuck::cast_slice(frame.trail_vertices);
        if !trail_bytes.is_empty() {
            let needed = trail_bytes.len() as u64;
            if needed > self.trail_buffer_bytes {
                let new_size = needed.next_power_of_two().max(self.trail_buffer_bytes * 2);
                self.trail_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("trail vertices (grown)"),
                    size: new_size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.trail_buffer_bytes = new_size;
            }
            queue.write_buffer(&self.trail_buffer, 0, trail_bytes);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hdr scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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

            pass.set_bind_group(0, &self.scene_bg, &[]);
            pass.set_pipeline(&self.bg_pipeline);
            pass.draw(0..3, 0..1);

            if !frame.ship_vertices.is_empty() {
                pass.set_pipeline(&self.rocket_pipeline);
                pass.set_vertex_buffer(0, self.ship_buffer.slice(..));
                pass.draw(0..frame.ship_vertices.len() as u32, 0..1);
            }
            if !frame.trail_vertices.is_empty() {
                pass.set_pipeline(&self.trail_pipeline);
                pass.set_vertex_buffer(0, self.trail_buffer.slice(..));
                pass.draw(0..frame.trail_vertices.len() as u32, 0..1);
            }
        }

        // Bright pass -> bloom_a.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bright pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_a_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.bright_pipeline);
            pass.set_bind_group(0, &self.bright_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // Horizontal blur -> bloom_b.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur h pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_b_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, &self.blur_h_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // Vertical blur -> bloom_a.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur v pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_a_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, &self.blur_v_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // Composite to the swapchain / screenshot target.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, &self.post_bg, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

fn create_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("render target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn create_bloom_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    params: &wgpu::Buffer,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params.as_entire_binding(),
            },
        ],
    })
}

fn create_post_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    scene_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    bloom_view: &wgpu::TextureView,
    post_uniform: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("post bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(scene_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(bloom_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: post_uniform.as_entire_binding(),
            },
        ],
    })
}

fn post_pipeline(
    device: &wgpu::Device,
    label: &str,
    shader: &wgpu::ShaderModule,
    fragment_entry: &str,
    layout: &wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: fragment_entry,
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
    })
}
