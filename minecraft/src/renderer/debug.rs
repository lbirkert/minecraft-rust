//! This module contains the debug overlay for minecraft.
//! We are using EGUI.

use egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor};

#[derive(Default)]
pub struct DebugSettings {
    pub show_chunk_updates: bool,
}

pub struct DebugOverlay {
    pub settings: DebugSettings,
    pub show: bool,

    egui_context: egui::Context,
    egui_renderer: Renderer,
    egui_state: egui_winit::State,
}

impl DebugOverlay {
    pub fn new(
        window: &winit::window::Window,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> Self {
        let egui_context = egui::Context::default();

        let egui_state = egui_winit::State::new(
            egui_context.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let egui_renderer = Renderer::new(device, format, RendererOptions::default());

        DebugOverlay {
            settings: Default::default(),

            egui_context,
            egui_renderer,
            egui_state,

            show: false,
        }
    }

    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        window: &winit::window::Window,
        size: &winit::dpi::PhysicalSize<u32>,
        view: &wgpu::TextureView,
    ) -> Vec<wgpu::CommandBuffer> {
        let input = self.egui_state.take_egui_input(window);

        let mut output = self.egui_context.run_ui(input, |ctx| {
            if self.show {
                egui::Window::new("Debug panel")
                    .default_pos([20.0, 20.0])
                    .show(ctx, |ui| {
                        ui.label("Rendered over your scene");

                        ui.checkbox(&mut self.settings.show_chunk_updates, "show chunk updates");

                        ui.label("Background color");
                    });
            }
        });

        self.egui_state
            .handle_platform_output(window, output.platform_output);

        let paint_jobs = self
            .egui_context
            .tessellate(output.shapes, output.pixels_per_point);

        let screen = ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: output.pixels_per_point,
        };

        // 1. Apply texture updates before rendering.
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                self.egui_renderer.update_texture(device, queue, *id, delta);
            }
        }

        // 2. Upload vertex/index buffers and prepare callbacks.
        let callback_buffers =
            self.egui_renderer
                .update_buffers(device, queue, encoder, &paint_jobs, &screen);

        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        self.egui_renderer
            .render(&mut pass.forget_lifetime(), &paint_jobs, &screen);

        for id in &output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        output.textures_delta.clear();

        callback_buffers
    }

    pub fn on_window_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> egui_winit::EventResponse {
        self.egui_state.on_window_event(window, event)
    }
}
