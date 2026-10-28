use std::{rc::Rc, sync::Arc, time::Instant};

use nalgebra::{Point3, Vector3};
use rayon::{ThreadPool, ThreadPoolBuilder};
use winit::{event_loop::OwnedDisplayHandle, window::Window};

use crate::{
    assets::AssetsLoader,
    camera::Camera,
    chunk::{ChunkMesher, ChunkRenderer},
    physics::{PlayerPhysics, raycast},
    primitives::{BlockData, Chunk, Player, World},
};

pub struct Game {
    instance: wgpu::Instance,
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,

    last_frame: Instant,
    last_meshing: Instant,
    is_cursor_grabbed: bool,

    thread_pool: Rc<ThreadPool>,

    player: Player,
    player_physics: PlayerPhysics,

    selected_block: Option<(Point3<i32>, Option<Vector3<f32>>)>,

    // shader stuff
    world: World,
    camera: Camera,
    chunk_renderer: ChunkRenderer,
    chunk_mesher: ChunkMesher,

    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,

    keys: GameKeys,
}

#[derive(Default)]
pub struct GameKeys {
    pub lmb: bool,
    pub rmb: bool,
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
}

impl Game {
    pub async fn new(display: OwnedDisplayHandle, window: Arc<Window>) -> Self {
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
        let thread_pool = Rc::new(ThreadPoolBuilder::new().build().unwrap());

        let world = World::new(thread_pool.clone(), 0);

        // add player
        let player = Player {
            pos: Point3::new(0.0, 100.0, 0.0),
            ..Default::default()
        };
        let player_physics = Default::default();

        let mut loader = AssetsLoader::new(".");
        loader.load_models();
        loader.load_textures();
        let assets = Arc::new(loader.finish());

        let camera = Camera::new(&device, size.clone());
        let chunk_renderer = ChunkRenderer::new(
            &device,
            &queue,
            &assets.model_registry.quads,
            &assets.texture_registry.atlas,
            &camera.camera_uniform_buffer,
            surface_format,
        );
        let chunk_mesher = ChunkMesher::new(thread_pool.clone(), assets);

        let (depth_texture, depth_view) =
            Self::create_depth_texture(&device, size.width, size.height);

        let game = Game {
            instance,
            window,
            device,
            queue,
            size,
            surface,
            surface_format,

            player,
            player_physics,

            last_frame: Instant::now(),
            last_meshing: Instant::now(),
            is_cursor_grabbed: false,

            selected_block: None,

            thread_pool,

            world,
            camera,
            chunk_renderer,
            chunk_mesher,

            depth_texture,
            depth_view,

            keys: GameKeys::default(),
        };

        // Configure surface for the first time
        game.configure_surface();

        game
    }

    pub fn place_block(&mut self) {
        match self.selected_block {
            Some((ref pos, Some(ref normal))) => {
                let pos = pos + normal.map(|f| f as i32);

                // check previous block
                let prev_block = self.world.get_block_data_fast(&pos);
                if prev_block.is_some_and(|b| !b.is_air()) {
                    return;
                }

                self.world.set_block_data_fast(&pos, BlockData::Stone);
                self.chunk_mesher.block_update(&pos);
            }
            _ => {}
        }
    }

    pub fn break_block(&mut self) {
        match self.selected_block {
            Some((ref pos, _)) => {
                // check previous block
                let prev_block = self.world.get_block_data_fast(&pos);
                if prev_block.is_some_and(|b| b.is_unbreakable()) {
                    return;
                }

                self.world.set_block_data_fast(pos, BlockData::Air);
                self.chunk_mesher.block_update(&pos);
            }
            _ => {}
        }
    }

    // update physics?
    pub fn update(&mut self, dt: f32) {
        let (sy, cy) = self.player.yaw.sin_cos();
        let forward = Vector3::new(sy, 0.0, -cy);
        let right = Vector3::new(cy, 0.0, sy);

        let mut dir = Vector3::zeros();
        if self.keys.forward {
            dir += forward;
        }
        if self.keys.backward {
            dir -= forward;
        }
        if self.keys.right {
            dir += right;
        }
        if self.keys.left {
            dir -= right;
        }

        if self.keys.up {
            self.player_physics.jump(&mut self.player);
        }

        if dir.norm_squared() > 0.0 {
            dir.normalize_mut();
        }

        self.player_physics.input = dir;
        self.player_physics.is_sprinting = self.keys.sprint;

        self.player_physics
            .update(&mut self.player, &self.world, dt);
    }

    pub fn process_key_event(&mut self, event: winit::event::KeyEvent) {
        let pressed = event.state == winit::event::ElementState::Pressed;

        let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key else {
            return;
        };

        match key_code {
            winit::keyboard::KeyCode::KeyW => self.keys.forward = pressed,
            winit::keyboard::KeyCode::KeyS => self.keys.backward = pressed,
            winit::keyboard::KeyCode::KeyA => self.keys.left = pressed,
            winit::keyboard::KeyCode::KeyD => self.keys.right = pressed,
            winit::keyboard::KeyCode::Space => self.keys.up = pressed,
            winit::keyboard::KeyCode::ControlLeft => self.keys.down = pressed,
            winit::keyboard::KeyCode::ShiftLeft => self.keys.sprint = pressed,
            winit::keyboard::KeyCode::Escape => self.set_cursor_grabbed(false),
            _ => {}
        }
    }

    pub fn set_cursor_grabbed(&mut self, grabbed: bool) {
        let cgrab = match grabbed {
            false => winit::window::CursorGrabMode::None,
            true => winit::window::CursorGrabMode::Locked,
        };

        self.window.set_cursor_grab(cgrab).unwrap();
        self.window.set_cursor_visible(!grabbed);
        self.is_cursor_grabbed = grabbed;
    }

    pub fn process_mouse_event(&mut self, dx: f64, dy: f64) {
        if !self.is_cursor_grabbed {
            return;
        }

        let sensitivity = 0.002;

        self.player.yaw += dx as f32 * sensitivity;
        self.player.pitch -= dy as f32 * sensitivity;

        let limit = std::f32::consts::FRAC_PI_2 - 0.01;
        self.player.pitch = self.player.pitch.clamp(-limit, limit);
    }

    pub fn process_mouse_input(
        &mut self,
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
    ) {
        if !self.is_cursor_grabbed && matches!(button, winit::event::MouseButton::Left) {
            self.set_cursor_grabbed(true);
            return;
        }

        let (last_lmb, last_rmb) = (self.keys.lmb, self.keys.rmb);
        let pressed = matches!(state, winit::event::ElementState::Pressed);
        match button {
            winit::event::MouseButton::Left => self.keys.lmb = pressed,
            winit::event::MouseButton::Right => self.keys.rmb = pressed,
            _ => {}
        }

        if !last_lmb && self.keys.lmb {
            self.break_block();
        }

        if !last_rmb && self.keys.rmb {
            self.place_block();
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.camera.screen_size = new_size;

        // reconfigure the surface
        self.configure_surface();

        (self.depth_texture, self.depth_view) =
            Self::create_depth_texture(&self.device, new_size.width, new_size.height);
    }

    pub fn render(&mut self) {
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

        // If you wanted to call any drawing commands, they would go here.
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now.clone();

        // update player physics
        self.update(dt);

        self.camera.write_uniform(&mut self.player, &mut self.queue);

        let pos = Point3::new(
            self.player.pos.x.floor() as i32,
            self.player.pos.y.floor() as i32,
            self.player.pos.z.floor() as i32,
        );

        self.world.chunk_loader.process_task_outputs();
        self.world.chunk_loader.update_chunks(&pos);
        self.chunk_mesher.update_chunks(&pos, &self.world);

        self.chunk_mesher.process_task_outputs(&self.device);

        self.selected_block = raycast::get_selected_block(&self.player, &self.world);

        let selected_block_id = self
            .selected_block
            .map(|b| Chunk::get_block_id(&b.0))
            .flatten();

        // render all meshed chunks
        self.chunk_renderer.write_buffer(
            &mut self.queue,
            self.chunk_mesher.meshes.values(),
            selected_block_id,
            self.chunk_mesher.meshes.len(),
        );
        for (i, mesh) in self.chunk_mesher.meshes.values().enumerate() {
            self.chunk_renderer.render(&mut renderpass, mesh, i as u64);
        }

        // End the renderpass.
        drop(renderpass);

        // Submit the command in the queue tote
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(surface_texture);

        self.window.request_redraw();
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

    fn configure_surface(&self) {
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
}
