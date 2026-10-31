use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::assets::TextureRegistry;

pub struct GuiRect {
    pub texture: String,
    pub z_index: i32,
    pub display_mode: DisplayMode,
}

pub enum DisplayMode {
    Center {
        center: [f32; 2],
        width: Option<f32>,
        height: Option<f32>,
        uv: Option<[f32; 4]>,
    },
}

pub struct GuiRenderer {
    vertex_buffer: wgpu::Buffer,
    vertex_count: usize,

    index_buffer: wgpu::Buffer,
    index_count: usize,

    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,

    // for textures
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,

    rects: Vec<GuiRect>,
}

#[repr(C)]
#[derive(Pod, Clone, Copy, Zeroable)]
struct Vertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
}

impl GuiRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &image::RgbaImage,
        camera_buffer: &wgpu::Buffer,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let (texture, view, sampler) = Self::create_texture(device, queue, image);

        let bind_group_layout = Self::create_bind_group_layout(device);
        let bind_group =
            Self::create_bind_group(device, &bind_group_layout, camera_buffer, &view, &sampler);

        let shader = device.create_shader_module(wgpu::include_wgsl!("./shaders/gui.wgsl"));
        let pipeline = Self::create_pipeline(device, &bind_group_layout, &shader, surface_format);

        let (vertex_buffer, vertex_count) = Self::create_vertex_buffer(&device, &[]);
        let (index_buffer, index_count) = Self::create_index_buffer(&device, &[]);

        Self {
            bind_group_layout,
            bind_group,

            pipeline,

            texture,
            view,
            sampler,

            vertex_buffer,
            vertex_count,

            index_buffer,
            index_count,

            rects: Default::default(),
        }
    }

    pub fn render(&mut self, renderpass: &mut wgpu::RenderPass) {
        if self.vertex_count == 0 {
            return;
        }

        renderpass.set_pipeline(&self.pipeline);
        renderpass.set_bind_group(0, &self.bind_group, &[]);
        renderpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        renderpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        renderpass.draw_indexed(0..self.index_count as u32, 0, 0..1);
    }

    pub fn clear(&mut self) {
        self.rects.truncate(0);
    }

    pub fn add(&mut self, rect: GuiRect) {
        self.rects.push(rect);
    }

    pub fn write_buffers(&mut self, device: &wgpu::Device, texture_registry: &TextureRegistry) {
        let mut vertices = Vec::with_capacity(self.rects.len() * 4);
        let mut indices = Vec::with_capacity(self.rects.len() * 6);

        self.rects.sort_by_key(|rect| rect.z_index);

        for (i, rect) in self.rects.iter().enumerate() {
            let atlas_uv = texture_registry
                .gui_atlas
                .textures
                .get(&rect.texture)
                .expect("GUI texture not found");

            let [atlas_u, atlas_v, atlas_w, atlas_h] = *atlas_uv;

            let DisplayMode::Center {
                center,
                width,
                height,
                uv,
            } = &rect.display_mode;

            // Use the full texture region unless a UV subregion was specified.
            let uv = (*uv).unwrap_or([0.0, 0.0, 1.0, 1.0]);

            // Convert the relative UV region into atlas coordinates.
            let uv = [
                atlas_u as f32 + uv[0] as f32 * atlas_w as f32,
                atlas_v as f32 + uv[1] as f32 * atlas_h as f32,
                uv[2] as f32 * atlas_w as f32,
                uv[3] as f32 * atlas_h as f32,
            ];

            let aspect_ratio = uv[2] / uv[3];

            let (width, height) = match (width, height) {
                (Some(w), Some(h)) => (*w, *h),
                (Some(w), None) => (*w, *w / aspect_ratio),
                (None, Some(h)) => (*h * aspect_ratio, *h),
                (None, None) => {
                    panic!("Center display mode requires width or height");
                }
            };

            let lower = [center[0] - width / 2.0, center[1] - height / 2.0];

            let upper = [center[0] + width / 2.0, center[1] + height / 2.0];

            // Bottom-left
            vertices.push(Vertex {
                pos: [lower[0], lower[1]],
                uv: [uv[0], uv[1] + uv[3]],
            });

            // Top-left
            vertices.push(Vertex {
                pos: [lower[0], upper[1]],
                uv: [uv[0], uv[1]],
            });

            // Top-right
            vertices.push(Vertex {
                pos: [upper[0], upper[1]],
                uv: [uv[0] + uv[2], uv[1]],
            });

            // Bottom-right
            vertices.push(Vertex {
                pos: [upper[0], lower[1]],
                uv: [uv[0] + uv[2], uv[1] + uv[3]],
            });

            let base = (i * 4) as u16;

            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        (self.vertex_buffer, self.vertex_count) = Self::create_vertex_buffer(device, &vertices);

        (self.index_buffer, self.index_count) = Self::create_index_buffer(device, &indices);
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

    fn create_vertex_buffer(device: &wgpu::Device, contents: &[Vertex]) -> (wgpu::Buffer, usize) {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: bytemuck::cast_slice(contents),
            usage: wgpu::BufferUsages::VERTEX,
        });
        (buffer, contents.len())
    }

    fn create_index_buffer(device: &wgpu::Device, contents: &[u16]) -> (wgpu::Buffer, usize) {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index buffer"),
            contents: bytemuck::cast_slice(contents),
            usage: wgpu::BufferUsages::INDEX,
        });
        (buffer, contents.len())
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        camera_buffer: &wgpu::Buffer,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gui bind group"),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
            layout,
        })
    }

    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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
            label: Some("gui pipeline layout"),
            bind_group_layouts: &[Some(bind_group_layout)],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gui pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }
}
