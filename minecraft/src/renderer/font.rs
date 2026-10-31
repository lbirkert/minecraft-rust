use std::{collections::HashMap, sync::Arc};

use bytemuck::{Pod, Zeroable};
use image::Pixel;
use wgpu::util::DeviceExt;

use crate::assets::Assets;

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    uv: [u32; 2],
    color: [f32; 4],
}

pub struct FontRenderer {
    assets: Arc<Assets>,
    ascii_glyphs: GlyphTable,
    vertices: Vec<Vertex>,

    vertex_buffer: wgpu::Buffer,
    vertex_count: usize,

    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,

    // for textures
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl FontRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        camera_buffer: &wgpu::Buffer,
        assets: Arc<Assets>,
    ) -> Self {
        // TODO: build glyph table
        let ascii_glyphs = GlyphTable::build(&assets);

        let (texture, view, sampler) =
            Self::create_texture(device, queue, &assets.texture_registry.font_atlas.atlas);

        let bind_group_layout = Self::create_bind_group_layout(device);
        let bind_group =
            Self::create_bind_group(device, &bind_group_layout, camera_buffer, &view, &sampler);

        let shader = device.create_shader_module(wgpu::include_wgsl!("./shaders/font.wgsl"));
        let pipeline = Self::create_pipeline(device, &bind_group_layout, &shader, surface_format);

        let (vertex_buffer, vertex_count) = Self::create_vertex_buffer(&device, &[]);

        Self {
            ascii_glyphs,
            assets,
            vertices: Default::default(),

            bind_group_layout,
            bind_group,

            pipeline,

            texture,
            view,
            sampler,

            vertex_buffer,
            vertex_count,
        }
    }

    pub fn render(&mut self, renderpass: &mut wgpu::RenderPass) {
        if self.vertex_count == 0 {
            return;
        }

        renderpass.set_pipeline(&self.pipeline);
        renderpass.set_bind_group(0, &self.bind_group, &[]);
        renderpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        renderpass.draw(0..self.vertex_count as u32, 0..1);
    }

    pub fn clear(&mut self) {
        self.vertices.truncate(0);
    }

    pub fn add(&mut self, text: &str, mut x: f32, y: f32, height: f32, color: [f32; 4]) {
        for c in text.chars() {
            let Some(&glyph) = self.ascii_glyphs.glyphs.get(&c) else {
                continue;
            };

            self.add_glyph(glyph, x, y, height, color);

            // advance by glyph width
            x += (glyph[2] as f32) * (height / 8.0);
            x += height / 8.0;
        }
    }

    pub fn add_centered(&mut self, text: &str, mut x: f32, y: f32, height: f32, color: [f32; 4]) {
        let len = self.get_length(text, height);
        self.add(text, x - len / 2.0, y - height / 2.0, height, color);
    }

    pub fn get_length(&self, text: &str, height: f32) -> f32 {
        let mut width = 0.0;

        for c in text.chars() {
            let Some(&glyph) = self.ascii_glyphs.glyphs.get(&c) else {
                continue;
            };
            width += (glyph[2] as f32) * (height / 8.0);
            width += height / 8.0;
        }

        width
    }

    pub fn add_glyph(&mut self, glyph: [u32; 4], x: f32, y: f32, height: f32, color: [f32; 4]) {
        let (x_min, y_min) = (x, y);
        println!("glyph width: {}", glyph[2]);
        let x_max = x + (glyph[2] as f32) * (height / 8.0);
        let y_max = y + height;

        let (u_min, v_min) = (glyph[0], glyph[1]);
        let (u_max, v_max) = (glyph[0] + glyph[2], glyph[1] + glyph[3]);

        self.vertices.extend([
            // 1st triangle:
            Vertex {
                pos: [x_min, y_min],
                uv: [u_min, v_max],
                color,
            },
            Vertex {
                pos: [x_min, y_max],
                uv: [u_min, v_min],
                color,
            },
            Vertex {
                pos: [x_max, y_max],
                uv: [u_max, v_min],
                color,
            },
            // 2nd triangle:
            Vertex {
                pos: [x_min, y_min],
                uv: [u_min, v_max],
                color,
            },
            Vertex {
                pos: [x_max, y_max],
                uv: [u_max, v_min],
                color,
            },
            Vertex {
                pos: [x_max, y_min],
                uv: [u_max, v_max],
                color,
            },
        ]);
    }

    pub fn write_buffers(&mut self, device: &wgpu::Device) {
        if self.vertices.is_empty() {
            self.vertex_count = 0;
            return;
        }

        (self.vertex_buffer, self.vertex_count) =
            Self::create_vertex_buffer(device, &self.vertices);
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
            label: Some("font bind group layout"),
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
            label: Some("font pipeline layout"),
            bind_group_layouts: &[Some(bind_group_layout)],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("font pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Uint32x2,
                        2 => Float32x4,
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

#[derive(Default)]
pub struct GlyphTable {
    pub glyphs: HashMap<char, [u32; 4]>,
}

impl GlyphTable {
    pub fn build(assets: &Assets) -> GlyphTable {
        let mut glyphs = GlyphTable::default();

        // for now just load ascii

        let ascii_uv = assets
            .texture_registry
            .font_atlas
            .textures
            .get("font/ascii")
            .unwrap();
        let image = &assets.texture_registry.font_atlas.atlas;

        const GLYPH_DIM: u32 = 8;

        let mut glyph: u32 = 0;
        for y in 0..16 {
            for x in 0..16 {
                let (start_x, start_y) = (ascii_uv[0] + x * GLYPH_DIM, ascii_uv[1] + y * GLYPH_DIM);

                let mut glyph_width = 0;
                for scan_x in 0..GLYPH_DIM {
                    let mut col_has_pixel = false;
                    for scan_y in 0..GLYPH_DIM {
                        let scan_x = scan_x + start_x;
                        let scan_y = scan_y + start_y;
                        let color = image[(scan_x, scan_y)];

                        if color.alpha() != 0 {
                            col_has_pixel = true;
                            break;
                        }
                    }

                    if col_has_pixel == false {
                        break;
                    }

                    glyph_width += 1;
                }

                if glyph as u8 == b' ' {
                    glyph_width = GLYPH_DIM / 3;
                }

                let glyph_uv = [start_x, start_y, glyph_width, GLYPH_DIM];

                glyphs.glyphs.insert((glyph as u8) as char, glyph_uv);
                glyph += 1;
            }
        }

        glyphs
    }
}
