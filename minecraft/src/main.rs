use std::{process::ExitCode, sync::Arc, thread::sleep, time::Duration};

use log::{debug, info};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::EventLoop,
    platform::pump_events::{EventLoopExtPumpEvents, PumpStatus},
    window::Window,
};

use crate::game::Game;

mod assets;
mod config;
mod game;
mod gui;
mod physics;
mod primitives;
mod renderer;
mod terrain;

#[derive(Default)]
struct App {
    game: Option<Game>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        debug!("(Re)creating window...");
        let window_attributes = Window::default_attributes().with_title("Minecraft in Rust");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.game = Some(pollster::block_on(Game::new(
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
        let game = self.game.as_mut().unwrap();
        if let winit::event::DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            game.process_mouse_event(dx, dy)
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let game = self.game.as_mut().unwrap();
        game.process_window_event(&event);
        match event {
            WindowEvent::CloseRequested => {
                info!("The close button was pressed; stopping");
                event_loop.exit();
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
