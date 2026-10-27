use std::{
    process::ExitCode,
    rc::Rc,
    sync::Arc,
    thread::sleep,
    time::{Duration, Instant},
};

use log::{debug, info};
use nalgebra::Point3;
use rayon::{ThreadPool, ThreadPoolBuilder};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{EventLoop, OwnedDisplayHandle},
    platform::pump_events::{EventLoopExtPumpEvents, PumpStatus},
    window::Window,
};

use crate::{
    assets::AssetsLoader,
    camera::Camera,
    chunk::{ChunkMesher, ChunkRenderer},
    physics::{PlayerPhysics, raycast},
    primitives::{Player, World},
};

mod assets;
mod camera;
mod chunk;
mod config;
mod physics;
mod primitives;

struct State {
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

    selected_block: Option<(u64, u32)>,

    // shader stuff
    world: World,
    camera: Camera,
    chunk_renderer: ChunkRenderer,
    chunk_mesher: ChunkMesher,

    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
}

impl State {
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

    async fn new(display: OwnedDisplayHandle, window: Arc<Window>) -> State {
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

        let state = State {
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
        };

        // Configure surface for the first time
        state.configure_surface();

        state
    }

    fn get_window(&self) -> &Window {
        &self.window
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

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.camera.screen_size = new_size;

        // reconfigure the surface
        self.configure_surface();

        (self.depth_texture, self.depth_view) =
            Self::create_depth_texture(&self.device, new_size.width, new_size.height);
    }

    fn render(&mut self) {
        // Create texture view.
        // NOTE: We must handle Timeout because the surface may be unavailable
        // (e.g., when the window is occluded on macOS).
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
        self.camera
            .update(&mut self.player, &mut self.player_physics);
        self.player_physics
            .update(&mut self.player, &self.world, dt);

        self.camera.write_uniform(&mut self.player, &mut self.queue);

        let pos = Point3::new(
            self.player.pos.x.floor() as i32,
            self.player.pos.y.floor() as i32,
            self.player.pos.z.floor() as i32,
        );

        self.world.chunk_loader.process_task_outputs();
        self.world.chunk_loader.update_chunks(&pos);

        self.chunk_mesher.process_task_outputs(&self.device);

        self.selected_block = raycast::get_selected_block(&self.player, &self.world);
        println!("selected block {:?}", self.selected_block);

        // we should batch this
        self.chunk_mesher.update_chunks(&pos, &self.world);
        // if (now - self.last_meshing).as_millis() > 2000 {
        //     self.last_meshing = now;
        // }

        // render all meshed chunks
        self.chunk_renderer.write_buffer(
            &mut self.queue,
            self.chunk_mesher.meshes.values(),
            self.selected_block,
            self.chunk_mesher.meshes.len(),
        );
        for (i, mesh) in self.chunk_mesher.meshes.values().enumerate() {
            self.chunk_renderer.render(&mut renderpass, &mesh, i as u64);
        }

        // End the renderpass.
        drop(renderpass);

        // Submit the command in the queue tote
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(surface_texture);
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        debug!("(Re)creating window...");
        let window_attributes = Window::default_attributes().with_title("Minecraft in Rust");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.state = Some(pollster::block_on(State::new(
            event_loop.owned_display_handle(),
            window.clone(),
        )));

        window.request_redraw();
    }

    fn device_event(
        &mut self,
        _: &winit::event_loop::ActiveEventLoop,
        _: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let state = self.state.as_mut().unwrap();
        match event {
            winit::event::DeviceEvent::MouseMotion { delta: (dx, dy) }
                if state.is_cursor_grabbed =>
            {
                state.camera.process_mouse_event(&mut state.player, dx, dy);
            }
            _ => {}
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let state = self.state.as_mut().unwrap();
        match event {
            WindowEvent::CloseRequested => {
                info!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                state.render();
                // Emits a new redraw requested event.
                state.get_window().request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                state.camera.process_key_event(event.clone());

                // ungrab cursor
                if state.is_cursor_grabbed
                    && matches!(
                        event.physical_key,
                        winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape)
                    )
                {
                    state
                        .window
                        .set_cursor_grab(winit::window::CursorGrabMode::None)
                        .unwrap();
                    state.window.set_cursor_visible(true);
                    state.is_cursor_grabbed = false;
                };
            }
            WindowEvent::MouseInput { button, .. } => {
                // grab cursor
                if !state.is_cursor_grabbed && matches!(button, winit::event::MouseButton::Left) {
                    state
                        .window
                        .set_cursor_grab(winit::window::CursorGrabMode::Locked)
                        .unwrap();
                    state.window.set_cursor_visible(false);
                    state.is_cursor_grabbed = true;
                }
            }
            WindowEvent::Resized(size) => {
                // Reconfigures the size of the surface. We do not re-render
                // here as this event is always followed up by redraw request.
                state.resize(size);
            }
            _ => (),
        }
    }
}

fn main() -> std::process::ExitCode {
    println!("Setting up logging...");
    simplelog::CombinedLogger::init(vec![
        simplelog::TermLogger::new(
            simplelog::LevelFilter::Info,
            simplelog::Config::default(),
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        ),
        simplelog::WriteLogger::new(
            simplelog::LevelFilter::Info,
            simplelog::Config::default(),
            std::fs::File::create("minecraft-rust.log").unwrap(),
        ),
    ])
    .unwrap();

    info!("░█▄█░▀█▀░█▀█░█▀▀░█▀▀░█▀▄░█▀█░█▀▀░▀█▀░░░░█▀▄░█▀▀");
    info!("░█░█░░█░░█░█░█▀▀░█░░░█▀▄░█▀█░█▀▀░░█░░░░░█▀▄░▀▀█");
    info!("░▀░▀░▀▀▀░▀░▀░▀▀▀░▀▀▀░▀░▀░▀░▀░▀░░░░▀░░▀░░▀░▀░▀▀▀");
    info!("Starting Minecraft in Rust...");

    debug!("Constructing app struct...");
    let mut app = App::default();

    debug!("Constructing event loop...");
    let mut event_loop = EventLoop::new().unwrap();

    debug!("Running event loop...");
    loop {
        let timeout = Some(Duration::ZERO);
        let status = event_loop.pump_app_events(timeout, &mut app);

        if let PumpStatus::Exit(exit_code) = status {
            break ExitCode::from(exit_code as u8);
        }

        // Sleep for 1/60 second to simulate application work
        //
        // Since `pump_events` doesn't block it will be important to
        // throttle the loop in the app somehow.
        debug!("Update()");
        sleep(Duration::from_millis(16));
    }
}
