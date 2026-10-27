use std::num::NonZeroU64;

use wgpu::util::DeviceExt;

use crate::{
    chunk::ChunkMesh,
    config::{CHUNK_OFFSET_Y, CHUNK_WIDTH_X, CHUNK_WIDTH_Z},
    primitives::Chunk,
};

pub struct ChunkRenderer {
    uniform_buffer: wgpu::Buffer, // resizeable
    uniform_stride: u64,
    uniform_size: u64,
    quad_buffer: wgpu::Buffer, // resizeable, mostly static

    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,

    // for textures
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl ChunkRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        quads: &[Quad],
        image: &image::RgbaImage,
        camera_buffer: &wgpu::Buffer,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let uniform_size = 1024; // the max. amount of meshes to draw in a single renderpass
        let (uniform_stride, uniform_buffer) = Self::create_uniform_buffer(device, uniform_size);
        let quad_buffer = Self::create_quad_buffer(device, quads);

        let (texture, view, sampler) = Self::create_texture(device, queue, image);

        let bind_group_layout = Self::create_bind_group_layout(&device);
        let bind_group = Self::create_bind_group(
            device,
            &bind_group_layout,
            camera_buffer,
            &uniform_buffer,
            &quad_buffer,
            &view,
            &sampler,
        );

        let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/chunk.wgsl"));
        let pipeline = Self::create_pipeline(device, &bind_group_layout, &shader, surface_format);

        Self {
            uniform_buffer,
            uniform_size,
            uniform_stride,

            quad_buffer,

            bind_group_layout,
            bind_group,

            pipeline,

            texture,
            view,
            sampler,
        }
    }

    pub fn write_buffer<'a>(
        &mut self,
        queue: &mut wgpu::Queue,
        chunks: impl Iterator<Item = &'a ChunkMesh>,
        selected_block: Option<(u64, u32)>,
        len: usize,
    ) {
        let mut bytes = vec![0u8; len * self.uniform_stride as usize];

        for (i, mesh) in chunks.enumerate() {
            let current_cid = Chunk::get_id(mesh.x, mesh.z);
            let selected_block = match selected_block {
                Some((cid, bid)) if current_cid == cid => {
                    println!("I HAVE SELECTION: {}", bid);
                    bid
                }
                _ => u32::MAX,
            };

            let start = i * self.uniform_stride as usize;
            let uniform = Uniform {
                bottom_left: [
                    (mesh.x * CHUNK_WIDTH_X) as f32,
                    CHUNK_OFFSET_Y as f32,
                    (mesh.z * CHUNK_WIDTH_Z) as f32,
                    0.0,
                ],
                selected_block,
                _padding: Default::default(),
            };
            let item_bytes = bytemuck::bytes_of(&uniform);

            bytes[start..start + item_bytes.len()].copy_from_slice(item_bytes);
        }

        queue.write_buffer(&self.uniform_buffer, 0, &bytes);
    }

    pub fn render(&mut self, renderpass: &mut wgpu::RenderPass, mesh: &ChunkMesh, idx: u64) {
        let offset = self.uniform_stride * idx;

        renderpass.set_pipeline(&self.pipeline);
        renderpass.set_bind_group(0, &self.bind_group, &[offset as u32]);
        renderpass.set_vertex_buffer(0, mesh.buffer.slice(..));

        let n_instances = mesh.buffer.size() as u32 / (4 * 2);
        renderpass.draw(0..6, 0..n_instances); // draw 6 verticies per quad
    }

    fn create_uniform_buffer(device: &wgpu::Device, n_chunks: u64) -> (u64, wgpu::Buffer) {
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let uniform_size = std::mem::size_of::<Uniform>() as u64;
        let stride = (uniform_size + alignment - 1) / alignment * alignment;

        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk uniforms"),
            size: stride * n_chunks,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        (stride, buf)
    }

    fn create_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &image::RgbaImage,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
        let (width, height) = image.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Atlas texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Atlas sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        (texture, view, sampler)
    }

    fn create_quad_buffer(device: &wgpu::Device, contents: &[Quad]) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk quad buffer"),
            contents: bytemuck::cast_slice(&contents),
            usage: wgpu::BufferUsages::STORAGE,
        })
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        camera_buffer: &wgpu::Buffer,
        uniform_buffer: &wgpu::Buffer,
        quad_buffer: &wgpu::Buffer,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let uniform_size = std::mem::size_of::<Uniform>() as u64;
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("chunk bind group"),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: NonZeroU64::new(uniform_size),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: quad_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            layout,
        })
    }

    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        let uniform_size = std::mem::size_of::<Uniform>() as u64;
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("chunk bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    visibility: wgpu::ShaderStages::VERTEX,
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(uniform_size),
                    },
                    visibility: wgpu::ShaderStages::VERTEX,
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    visibility: wgpu::ShaderStages::VERTEX,
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn create_pipeline(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        shader: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("chunk pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("chunk pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 8,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Uint32,
                        1 => Uint32,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        pipeline
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    block_id: u32,
    quad_id: u32,
}

impl Instance {
    pub fn new(quad_id: u32, bx: i32, by: i32, bz: i32) -> Self {
        let block_id = bx + (bz + by * CHUNK_WIDTH_Z) * CHUNK_WIDTH_X;

        Self {
            quad_id,
            block_id: block_id as u32,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Quad {
    pub v1: QuadVertex,
    pub v2: QuadVertex,
    pub v3: QuadVertex,
    pub v4: QuadVertex,
    pub tint: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub pos: u32,
    pub uv: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniform {
    pub bottom_left: [f32; 4],
    // pub tint_buffer: [[f32; 4]; 2], TODO: for custom tinting
    pub selected_block: u32,
    pub _padding: [u32; 3],
}

impl QuadVertex {
    fn pack_uv(u: f32, v: f32) -> u32 {
        let u = (u * 16.0).round().clamp(0.0, 65535.0) as u32;
        let v = (v * 16.0).round().clamp(0.0, 65535.0) as u32;

        u | (v << 16)
    }

    pub fn new(x: f32, y: f32, z: f32, uvx: f32, uvy: f32) -> Self {
        let px = (x.clamp(0.0, 1.0) * 255f32).round() as u32;
        let py = ((y.clamp(0.0, 1.0) * 255f32).round() as u32) << 8;
        let pz = ((z.clamp(0.0, 1.0) * 255f32).round() as u32) << 16;
        let pos = px | py | pz;
        let uv = Self::pack_uv(uvx, uvy);
        Self { pos, uv }
    }
}
