use std::{rc::Rc, sync::Arc, time::Instant};

use nalgebra::{Point3, Vector3};
use rayon::{ThreadPool, ThreadPoolBuilder};
use winit::{event::WindowEvent, event_loop::OwnedDisplayHandle, window::Window};

use crate::{
    assets::{Assets, loader::AssetsLoader},
    gui::{
        container::Container, render_button, render_crosshair, render_items_container,
        render_selected_item,
    },
    physics::{PlayerPhysics, raycast},
    primitives::{BlockData, Chunk, Player, World},
    renderer::Renderer,
    terrain::TerrainMesher,
};

pub struct Game {
    window: Arc<Window>,

    last_frame: Instant,
    last_meshing: Instant,

    is_cursor_grabbed: bool,
    is_paused: bool,

    thread_pool: Rc<ThreadPool>,

    assets: Arc<Assets>,

    player: Player,
    player_physics: PlayerPhysics,

    selected_block: Option<(Point3<i32>, Option<Vector3<f32>>)>,

    // shader stuff
    world: World, // does this make sense?
    terrain_mesher: TerrainMesher,

    renderer: Renderer,

    keys: GameKeys,

    container: Option<Container>,
}

#[derive(Default, Clone)]
pub struct GameKeys {
    pub key_e: bool,
    pub escape: bool,
    pub f1: bool,
    pub lmb: bool,
    pub rmb: bool,
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,

    pub digit1: bool,
    pub digit2: bool,
    pub digit3: bool,
    pub digit4: bool,
    pub digit5: bool,
    pub digit6: bool,
    pub digit7: bool,
    pub digit8: bool,
    pub digit9: bool,
}

impl Game {
    pub async fn new(display: OwnedDisplayHandle, window: Arc<Window>) -> Self {
        // resource packs here:
        let loader = AssetsLoader::new(vec![
            "resources/default".into(),
            //"resources/Goldbite-1".into(),
        ]);
        let assets = Arc::new(Assets::load(loader));

        let mut renderer = Renderer::new(display, window.clone(), assets.clone()).await;

        let thread_pool = Rc::new(ThreadPoolBuilder::new().build().unwrap());
        let world = World::new(thread_pool.clone(), 0);

        // add player
        let player = Player {
            pos: Point3::new(0.0, 100.0, 0.0),
            ..Default::default()
        };
        let player_physics = Default::default();

        let terrain_mesher = TerrainMesher::new(thread_pool.clone(), assets.clone());

        renderer.configure_surface();

        Game {
            window,

            player,
            player_physics,
            world,
            assets,
            thread_pool,
            renderer,

            // debug_overlay,
            terrain_mesher,

            last_frame: Instant::now(),
            last_meshing: Instant::now(),

            is_cursor_grabbed: false,
            is_paused: true,

            selected_block: None,

            keys: GameKeys::default(),

            container: None,
        }
    }

    pub fn place_block(&mut self) {
        let Some(block_data) = self.player.inventory.selected().get_block_data() else {
            return;
        };

        if let Some((ref pos, Some(ref normal))) = self.selected_block {
            let pos = pos + normal.map(|f| f as i32);

            // check previous block
            let prev_block = self.world.get_block_data_fast(&pos);
            if prev_block.is_some_and(|b| !b.is_air()) {
                return;
            }

            self.world.set_block_data_fast(&pos, block_data);
            self.terrain_mesher.block_update(&pos);
        }
    }

    pub fn break_block(&mut self) {
        if let Some((ref pos, _)) = self.selected_block {
            // check previous block
            let prev_block = self.world.get_block_data_fast(pos);
            if prev_block.is_some_and(|b| b.is_unbreakable()) {
                return;
            }

            self.world.set_block_data_fast(pos, BlockData::Air);
            self.terrain_mesher.block_update(pos);
        }
    }

    // TODO: rename to update physics?
    pub fn update(&mut self, dt: f32) {
        let (sy, cy) = self.player.yaw.sin_cos();
        let forward = Vector3::new(sy, 0.0, -cy);
        let right = Vector3::new(cy, 0.0, sy);

        let mut dir = Vector3::zeros();
        if self.is_cursor_grabbed {
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
        }

        if dir.norm_squared() > 0.0 {
            dir.normalize_mut();
        }

        self.player_physics.input = dir;
        self.player_physics.is_sprinting = self.keys.sprint;

        self.player_physics
            .update(&mut self.player, &self.world, dt);
    }

    pub fn process_key_event(&mut self, event: &winit::event::KeyEvent) {
        let pressed = event.state == winit::event::ElementState::Pressed;

        let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key else {
            return;
        };

        let old_keys = self.keys.clone();

        match key_code {
            winit::keyboard::KeyCode::KeyW => self.keys.forward = pressed,
            winit::keyboard::KeyCode::KeyS => self.keys.backward = pressed,
            winit::keyboard::KeyCode::KeyA => self.keys.left = pressed,
            winit::keyboard::KeyCode::KeyD => self.keys.right = pressed,
            winit::keyboard::KeyCode::Space => self.keys.up = pressed,
            winit::keyboard::KeyCode::ControlLeft => self.keys.down = pressed,
            winit::keyboard::KeyCode::ShiftLeft => self.keys.sprint = pressed,
            winit::keyboard::KeyCode::Escape => self.keys.escape = pressed,
            winit::keyboard::KeyCode::F1 => self.keys.f1 = pressed,
            winit::keyboard::KeyCode::KeyE => self.keys.key_e = pressed,

            winit::keyboard::KeyCode::Digit1 => self.keys.digit1 = pressed,
            winit::keyboard::KeyCode::Digit2 => self.keys.digit2 = pressed,
            winit::keyboard::KeyCode::Digit3 => self.keys.digit3 = pressed,
            winit::keyboard::KeyCode::Digit4 => self.keys.digit4 = pressed,
            winit::keyboard::KeyCode::Digit5 => self.keys.digit5 = pressed,
            winit::keyboard::KeyCode::Digit6 => self.keys.digit6 = pressed,
            winit::keyboard::KeyCode::Digit7 => self.keys.digit7 = pressed,
            winit::keyboard::KeyCode::Digit8 => self.keys.digit8 = pressed,
            winit::keyboard::KeyCode::Digit9 => self.keys.digit9 = pressed,
            _ => {}
        }

        if !old_keys.f1 && self.keys.f1 {
            self.toggle_debug_overlay();
        }

        if !old_keys.key_e && self.keys.key_e {
            self.toggle_inventory();
        }

        let digits = [
            (old_keys.digit1, self.keys.digit1),
            (old_keys.digit2, self.keys.digit2),
            (old_keys.digit3, self.keys.digit3),
            (old_keys.digit4, self.keys.digit4),
            (old_keys.digit5, self.keys.digit5),
            (old_keys.digit6, self.keys.digit6),
            (old_keys.digit7, self.keys.digit7),
            (old_keys.digit8, self.keys.digit8),
            (old_keys.digit9, self.keys.digit9),
        ];

        for (i, (prev, curr)) in digits.iter().enumerate() {
            if !prev && *curr {
                self.player.inventory.selected_slot = i;
            }
        }

        if !old_keys.escape && self.keys.escape {
            if self.container.is_some() {
                self.container = None;
            } else {
                self.is_paused = true;
            }
            self.update_cursor_grabbed();
        }
    }

    pub fn toggle_inventory(&mut self) {
        self.container = match self.container.is_some() {
            true => None,
            false => Some(Container::InventoryCreative),
        };
        self.update_cursor_grabbed();
    }

    pub fn toggle_debug_overlay(&mut self) {
        self.renderer.debug_overlay.show = !self.renderer.debug_overlay.show;
    }

    pub fn update_cursor_grabbed(&mut self) {
        let grabbed = !self.is_paused && self.container.is_none();
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

    pub fn process_window_event(&mut self, event: &WindowEvent) {
        let response = self
            .renderer
            .debug_overlay
            .on_window_event(&self.window, &event);

        if response.consumed {
            return;
        }

        match event {
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.process_key_event(event);
            }
            WindowEvent::MouseInput { button, state, .. } => {
                self.process_mouse_input(*button, *state);
            }
            WindowEvent::Resized(size) => {
                // Reconfigures the size of the surface. We do not re-render
                // here as this event is always followed up by redraw request.
                self.resize(*size);
            }
            _ => {}
        }
    }

    pub fn process_mouse_input(
        &mut self,
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
    ) {
        if self.is_paused && matches!(button, winit::event::MouseButton::Left) {
            self.is_paused = false;
            self.update_cursor_grabbed();
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
        self.renderer.resize(new_size);
    }

    pub fn render(&mut self) {
        self.renderer.clear();

        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        // update player physics
        self.update(dt);

        self.renderer.update_camera(&self.player);

        let pos = Point3::new(
            self.player.pos.x.floor() as i32,
            self.player.pos.y.floor() as i32,
            self.player.pos.z.floor() as i32,
        );

        self.world.chunk_loader.process_task_outputs();
        self.world.chunk_loader.update_chunks(&pos);
        self.terrain_mesher.update_chunks(&pos, &self.world);
        self.terrain_mesher
            .process_task_outputs(&self.renderer.device);

        if let Some(ref container) = self.container {
            container.render(&mut self.renderer);
        }

        if self.is_paused {
            self.renderer.font_renderer.add_centered(
                "Game is Paused",
                0.0,
                0.0,
                160.0,
                [1.0, 1.0, 0.0, 1.0],
            );

            render_button(&mut self.renderer, "example button", 0.0, -100.0);
        } else {
            render_crosshair(&mut self.renderer);
            render_items_container(&mut self.renderer);
            render_selected_item(
                &mut self.renderer,
                self.player.inventory.selected_slot as i32,
            );
        }

        self.selected_block = raycast::get_selected_block(&self.player, &self.world);
        let selected_block_id = self.selected_block.and_then(|b| Chunk::get_block_id(&b.0));

        // TODO: add stuff to the renderers here
        self.renderer
            .render(&self.terrain_mesher.meshes, selected_block_id);
    }
}
