mod state;

use futures::executor::block_on;
use std::time::Instant;
use winit::event::{Event, WindowEvent};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use state::{ImguiState, State};
fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("learn_wgpu")
        .build(&event_loop)
        .unwrap();

    let mut imgui_state = ImguiState::new(&window, 1.0);
    let mut state = block_on(State::new(&window, &mut imgui_state.context));

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::NewEvents(_) => {}
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() && !state.input(event) => match event {
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
                    state.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged {
                    new_inner_size,
                    scale_factor,
                    ..
                } => {
                    state.scale_factor = *scale_factor;
                    state.resize(**new_inner_size);
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                let delta_t = state.last_frame.elapsed();
                state.last_frame = Instant::now();

                state.update(delta_t); // game loop

                let ui = imgui_state.prepare(&window, delta_t);
                state.render(ui, delta_t);
            }
            _ => {}
        }
        imgui_state
            .platform
            .handle_event(imgui_state.context.io_mut(), &window, &event);
    });
}
