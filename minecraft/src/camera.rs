use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, Vector3};

use crate::{physics::PlayerPhysics, primitives::Player};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

pub struct Camera {
    pub fov: f32,
    pub keys: CameraKeys,
    pub move_speed: f32,
    pub speed_modifier: f32,

    pub camera_uniform_buffer: wgpu::Buffer,
    pub screen_size: winit::dpi::PhysicalSize<u32>,
}

#[derive(Default)]
pub struct CameraKeys {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
}

impl Camera {
    pub fn new(device: &wgpu::Device, screen_size: winit::dpi::PhysicalSize<u32>) -> Self {
        let camera_uniform_buffer = Self::create_camera_uniform_buffer(device);

        Self {
            fov: 90.0,
            move_speed: 5.0,
            speed_modifier: 2.0,
            screen_size,
            keys: CameraKeys::default(),
            camera_uniform_buffer,
        }
    }

    pub fn write_uniform(&self, player: &Player, queue: &mut wgpu::Queue) {
        let view_proj = self.calc_view_proj(player).into();

        queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&CameraUniform { view_proj }),
        );
    }

    fn create_camera_uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera buffer"),
            size: 4 * 4 * 4, // mat4x4<f32>,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn calc_view_proj(&self, player: &Player) -> Matrix4<f32> {
        let (sy, cy) = player.yaw.sin_cos();
        let (sp, cp) = player.pitch.sin_cos();

        // Camera looks along -Z when yaw = pitch = 0.
        let dir = Vector3::new(sy * cp, sp, -cy * cp).normalize();

        let eye = Player::get_eye_pos(&player.pos);
        let target = eye + dir;

        let view = Isometry3::look_at_rh(&eye, &target, &Vector3::y()).to_homogeneous();

        let aspect = self.screen_size.width as f32 / self.screen_size.height as f32;
        let projection = Perspective3::new(
            aspect, self.fov, // radians
            0.01,     // near plane
            1000.0,   // far plane
        );

        projection.as_matrix() * view
    }
}
