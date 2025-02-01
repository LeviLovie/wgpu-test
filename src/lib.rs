use tracing::{debug, error, info, info_span, trace, warn};
use winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

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

    info!("Running event loop");
    let _ = event_loop.run(move |event, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window().id() => {
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

                        state.update();
                        match state.render() {
                            Ok(_) => {}

                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                state.resize(state.size)
                            }

                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                error!("OutOfMemory");
                                control_flow.exit();
                            }

                            Err(wgpu::SurfaceError::Timeout) => {
                                warn!("Surface timeout")
                            }
                        }
                    }

                    _ => {}
                }
            }
        }
        _ => {}
    });
}
