pub mod debug;
pub mod font;
pub mod gui;
pub mod terrain;

pub use debug::DebugOverlay;
pub use font::FontRenderer;
pub use gui::GuiRenderer;
pub use terrain::TerrainRenderer;

use crate::assets::Assets;
use crate::primitives::Player;
use crate::terrain::TerrainMesh;
use nalgebra::{Isometry3, Matrix4, Perspective3, Vector3};
use std::collections::HashMap;
use std::sync::Arc;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    screen_size: [f32; 2],
}

pub struct Renderer {
    pub terrain_renderer: TerrainRenderer,
    pub gui_renderer: GuiRenderer,
    pub font_renderer: FontRenderer,
    pub debug_overlay: DebugOverlay,

    pub window: Arc<Window>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,

    assets: Arc<Assets>,

    fov: f32,
    size: winit::dpi::PhysicalSize<u32>,

    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,

    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,

    camera_uniform_buffer: wgpu::Buffer,
}

impl Renderer {
    pub async fn new(
        display: OwnedDisplayHandle,
        window: Arc<Window>,
        assets: Arc<Assets>,
    ) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(display),
        ));
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let size = window.inner_size();

        let surface = instance.create_surface(window.clone()).unwrap();
        let cap = surface.get_capabilities(&adapter);
        let surface_format = cap.formats[0];

        let camera_uniform_buffer = Self::create_camera_uniform_buffer(&device);
        let terrain_renderer = TerrainRenderer::new(
            &device,
            &queue,
            &assets.model_registry.quads,
            &assets.texture_registry.terrain_atlas.atlas,
            &camera_uniform_buffer,
            surface_format,
        );

        let gui_renderer = GuiRenderer::new(
            &device,
            &queue,
            &assets.texture_registry.gui_atlas.atlas,
            &camera_uniform_buffer,
            surface_format,
        );

        let font_renderer = FontRenderer::new(
            &device,
            &queue,
            surface_format,
            &camera_uniform_buffer,
            assets.clone(),
        );

        let (depth_texture, depth_view) =
            Self::create_depth_texture(&device, size.width, size.height);

        let debug_overlay = DebugOverlay::new(&window, &device, surface_format.clone());

        Self {
            debug_overlay,
            gui_renderer,
            terrain_renderer,
            font_renderer,

            assets,

            fov: 90.0,
            camera_uniform_buffer,

            instance,
            window,
            device,
            queue,
            size,
            surface,
            surface_format,

            depth_texture,
            depth_view,
        }
    }

    pub fn clear(&mut self) {
        self.font_renderer.clear();
        self.gui_renderer.clear();
    }

    pub fn render(
        &mut self,
        meshes: &HashMap<u64, TerrainMesh>,
        selected_block_id: Option<(i32, i32, u32)>,
    ) {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => return,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                drop(texture);
                self.configure_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                unreachable!("No error scope registered, so validation errors will panic")
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(self.window.clone()).unwrap();
                self.configure_surface();
                return;
            }
        };
        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                // Without add_srgb_suffix() the image we will be working with
                // might not be "gamma correct".
                format: Some(self.surface_format.add_srgb_suffix()),
                ..Default::default()
            });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        // Create the renderpass which will clear the screen.
        let mut renderpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
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
            multiview_mask: None,
        });

        // // If you wanted to call any drawing commands, they would go here.

        self.gui_renderer
            .write_buffers(&self.device, &self.assets.texture_registry); // TODO: simplify
        self.font_renderer.write_buffers(&self.device);

        // render all meshed chunks
        self.terrain_renderer.write_buffer(
            &mut self.queue,
            meshes.values(),
            selected_block_id,
            meshes.len(),
        );
        for (i, mesh) in meshes.values().enumerate() {
            self.terrain_renderer
                .render(&mut renderpass, &mesh, i as u64);
        }

        self.gui_renderer.render(&mut renderpass);
        self.font_renderer.render(&mut renderpass);

        // End the renderpass.
        drop(renderpass);

        let callback_buffers = self.debug_overlay.render(
            &self.queue,
            &self.device,
            &mut encoder,
            &self.window,
            &self.size,
            &texture_view,
        );

        // Submit the command in the queue tote
        self.queue.submit(
            callback_buffers
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );

        self.window.pre_present_notify();
        self.queue.present(surface_texture);

        self.window.request_redraw();
    }

    pub fn update_camera(&self, player: &Player) {
        let view_proj = self.calc_view_proj(player).into();

        self.queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&CameraUniform {
                view_proj,
                screen_size: [self.size.width as f32, self.size.height as f32],
            }),
        );
    }

    fn calc_view_proj(&self, player: &Player) -> Matrix4<f32> {
        let (sy, cy) = player.yaw.sin_cos();
        let (sp, cp) = player.pitch.sin_cos();

        // Camera looks along -Z when yaw = pitch = 0.
        let dir = Vector3::new(sy * cp, sp, -cy * cp).normalize();

        let eye = Player::get_eye_pos(&player.pos);
        let target = eye + dir;

        let view = Isometry3::look_at_rh(&eye, &target, &Vector3::y()).to_homogeneous();

        let aspect = self.size.width as f32 / self.size.height as f32;
        let projection = Perspective3::new(
            aspect, self.fov, // radians
            0.01,     // near plane
            1000.0,   // far plane
        );

        projection.as_matrix() * view
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;

        self.configure_surface();

        (self.depth_texture, self.depth_view) =
            Self::create_depth_texture(&self.device, self.size.width, self.size.height);
    }

    pub fn configure_surface(&mut self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            // Request compatibility with the sRGB-format texture view we‘re going to create later.
            view_formats: vec![self.surface_format.add_srgb_suffix()],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.surface.configure(&self.device, &surface_config);
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
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

        let view = texture.create_view(&Default::default());
        (texture, view)
    }

    fn create_camera_uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera buffer"),
            size: 80, // mat4x4<f32> + f32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}
