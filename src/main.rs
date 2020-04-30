mod state;

use futures::executor::block_on;
use imgui::{im_str, Condition, Context, FontSource, Ui};
use imgui_wgpu::Renderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::time::{Duration, Instant};
use winit::event::{Event, WindowEvent};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use state::State;

fn init_imgui(
    window: &winit::window::Window,
    state: &mut State,
) -> (Context, WinitPlatform, Renderer) {
    let mut imgui = Context::create();

    let mut platform = WinitPlatform::init(&mut imgui);
    platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);

    let font_size = (13.0 * state.scale_factor) as f32;
    imgui.io_mut().font_global_scale = (1.0 / state.scale_factor) as f32;

    imgui.fonts().add_font(&[FontSource::DefaultFontData {
        config: Some(imgui::FontConfig {
            oversample_h: 1,
            pixel_snap_h: true,
            size_pixels: font_size,
            ..Default::default()
        }),
    }]);

    let renderer = Renderer::new(
        &mut imgui,
        &state.device,
        &mut state.queue,
        state.sc_desc.format,
        Some(wgpu::Color::default()),
    );

    (imgui, platform, renderer)
}

fn build_ui(ui: &Ui, delta_t: Duration) {
    imgui::Window::new(im_str!("Hello world"))
        .size([300.0, 100.0], Condition::FirstUseEver)
        .build(ui, || {
            ui.text(im_str!("Hello world!"));
            ui.text(im_str!("This is imgui-rs on WGPU!"));
            ui.separator();
            let mouse_pos = ui.io().mouse_pos;
            ui.text(im_str!(
                "Mouse Position: ({:.1},{:.1})",
                mouse_pos[0],
                mouse_pos[1]
            ));
        });

    imgui::Window::new(im_str!("Hello too"))
        .size([200.0, 50.0], Condition::FirstUseEver)
        .position([400.0, 200.0], Condition::FirstUseEver)
        .build(&ui, || {
            ui.text(im_str!("Frametime: {:?}", delta_t));
        });
}

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("learn_wgpu")
        .build(&event_loop)
        .unwrap();

    let mut state = block_on(State::new(&window));

    let (mut imgui, mut platform, mut renderer) = init_imgui(&window, &mut state);

    let mut last_frame = Instant::now();
    let mut last_cursor = None;

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
            Event::RedrawRequested(_) => {
                state.update();
                state.render();
            }
            Event::RedrawEventsCleared => {
                let delta_t = last_frame.elapsed();
                last_frame = imgui.io_mut().update_delta_time(last_frame);

                let frame = match state.swap_chain.get_next_texture() {
                    Ok(frame) => frame,
                    Err(e) => {
                        eprintln!("dropped frame: {:?}", e);
                        return;
                    }
                };

                let ui = imgui.frame();

                build_ui(&ui, delta_t);

                // Render UI
                let mut encoder: wgpu::CommandEncoder = state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                if last_cursor != Some(ui.mouse_cursor()) {
                    last_cursor = Some(ui.mouse_cursor());
                    platform.prepare_render(&ui, &window);
                }

                platform.prepare_render(&ui, &window);
                renderer
                    .render(ui.render(), &state.device, &mut encoder, &frame.view)
                    .expect("Rendering failed");
                state.queue.submit(&[encoder.finish()]);
            }
            Event::MainEventsCleared => {
                platform
                    .prepare_frame(imgui.io_mut(), &window)
                    .expect("Failed to prepare frame");
                window.request_redraw();
            }
            _ => {}
        }
        platform.handle_event(imgui.io_mut(), &window, &event);
    });
}
