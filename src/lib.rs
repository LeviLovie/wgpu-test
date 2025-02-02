use egui_winit::winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};
use tracing::{debug, error, info, info_span, trace, warn};

pub mod camera;
pub mod gui;
pub mod state;
pub mod texture;

use state::State;

pub async fn run() {
    tracing_subscriber::fmt::init();
    info!("Starting up");

    let event_loop;
    let window;
    let mut state;
    {
        let span = info_span!("initialization");
        let _enter = span.enter();

        trace!("Creating event loop and window");
        event_loop = match EventLoop::new() {
            Ok(event_loop) => {
                trace!("Event loop created");
                event_loop
            }
            Err(e) => {
                error!("Failed to create event loop: {:?}", e);
                panic!();
            }
        };
        window = match WindowBuilder::new()
            .with_title("With the Wind")
            .build(&event_loop)
        {
            Ok(window) => {
                trace!("Window created");
                window
            }
            Err(e) => {
                error!("Failed to create window: {:?}", e);
                panic!();
            }
        };
        debug!("Event loop and window created");

        trace!("Creating state");
        state = State::new(&window).await;
        debug!("State created");
        info!("Initialization complete");
    }
    let mut surface_configured = false;
    let mut last_render_time = std::time::Instant::now();

    info!("Running event loop");
    let _ = event_loop.run(move |event, control_flow| match event {
        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion{ delta, },
            .. // We're not using device_id currently
        } => if state.mouse_pressed {
            state.camera_controller.process_mouse(delta.0, delta.1)
        }
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window().id() && !state.input(event) => {
            if !state.input(event) {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state: ElementState::Pressed,
                                physical_key: PhysicalKey::Code(KeyCode::Escape),
                                ..
                            },
                        ..
                    } => control_flow.exit(),

                    WindowEvent::Resized(physical_size) => {
                        surface_configured = true;
                        state.resize(*physical_size);
                    }

                    WindowEvent::RedrawRequested => {
                        state.window().request_redraw();
                        if !surface_configured {
                            return;
                        }

                        let now = std::time::Instant::now();
                        let dt = now - last_render_time;
                        last_render_time = now;
                        state.status.delta = dt.as_micros();
                        state.status.fps = 1_000_000.0 / dt.as_micros() as f32;
                        state.status.fps_avg =
                            0.95 * state.status.fps_avg + 0.05 * state.status.fps;
                        state.update(dt);
                        match state.render() {
                            Ok(_) => {}

                            Err(
                                egui_wgpu::wgpu::SurfaceError::Lost
                                | egui_wgpu::wgpu::SurfaceError::Outdated,
                            ) => state.resize(state.size),

                            Err(egui_wgpu::wgpu::SurfaceError::OutOfMemory) => {
                                error!("OutOfMemory");
                                control_flow.exit();
                            }

                            Err(egui_wgpu::wgpu::SurfaceError::Timeout) => {
                                warn!("Surface timeout")
                            }
                        }
                    }

                    _ => {}
                }
            }
            state.egui.handle_input(&mut state.window, event);
        }
        _ => {}
    });
}
