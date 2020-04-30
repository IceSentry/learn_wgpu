mod input;
mod renderer;

use futures::executor::block_on;
use serde_derive::Deserialize;
use std::time::Instant;
use winit::event::{Event, WindowEvent};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use input::handle_input;
use renderer::{ImguiState, Renderer};

#[derive(Deserialize)]
pub struct Config {
    debug: DebugKeys,
}

#[derive(Deserialize)]
pub struct DebugKeys {
    imgui: bool,
    glyph: bool,
}

fn main() {
    let config: Config =
        toml::from_str(include_str!("Config.toml")).expect("failed to parse config.toml");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("learn_wgpu")
        .build(&event_loop)
        .unwrap();

    let mut imgui_state = ImguiState::new(&window, 1.0);
    let mut renderer = block_on(Renderer::new(&window, &mut imgui_state.context));

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::NewEvents(_) => {}
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() && !handle_input(event, &mut renderer) => match event {
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                }
                | WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(physical_size) => {
                    renderer.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged {
                    new_inner_size,
                    scale_factor,
                    ..
                } => {
                    renderer.scale_factor = *scale_factor;
                    renderer.resize(**new_inner_size);
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                let delta_t = renderer.last_frame.elapsed();
                renderer.last_frame = Instant::now();

                // update(delta_t)

                let ui = imgui_state.prepare(&window, delta_t);
                renderer.render(ui, delta_t, &config);
            }
            _ => {}
        }
        imgui_state
            .platform
            .handle_event(imgui_state.context.io_mut(), &window, &event);
    });
}
