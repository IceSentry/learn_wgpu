use bevy::{
    ecs::system::SystemState,
    input::mouse::{MouseButtonInput, MouseMotion, MouseWheel},
    prelude::*,
    winit::WinitWindows,
};
use winit::{
    event::{DeviceId, ModifiersState},
    event_loop::{EventLoop, EventLoopWindowTarget},
};

use crate::renderer::{RenderPhase, WgpuRenderer};

pub struct EguiPlugin;

pub struct EguiRenderPhase<'w> {
    #[allow(clippy::type_complexity)]
    state: SystemState<(
        Res<'w, WgpuRenderer>,
        Res<'w, egui_wgpu::renderer::ScreenDescriptor>,
        NonSend<'w, egui::Context>,
        NonSendMut<'w, egui_wgpu::renderer::RenderPass>,
        ResMut<'w, EguiWinitPlatform>,
        Res<'w, Windows>,
        NonSend<'w, WinitWindows>,
    )>,
    paint_jobs: Vec<egui::ClippedPrimitive>,
}

struct EguiWinitPlatform(egui_winit::State);

impl Plugin for EguiPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup.exclusive_system())
            .add_system_to_stage(CoreStage::PreUpdate, begin_frame)
            .add_system(hello)
            .add_system(handle_mouse_events);
    }
}

#[allow(clippy::type_complexity)]
fn setup(world: &mut World) {
    let renderer = world.resource::<WgpuRenderer>();
    let windows = world.resource::<Windows>();

    let pass = egui_wgpu::renderer::RenderPass::new(
        &renderer.device,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        1,
    );

    let window = windows.primary();
    let desc = egui_wgpu::renderer::ScreenDescriptor {
        size_in_pixels: [window.width() as u32, window.height() as u32],
        pixels_per_point: window.scale_factor() as f32,
    };

    let platform = egui_winit::State::new_with_wayland_display(None);

    let initial_state = SystemState::new(world);

    world.insert_non_send_resource(pass);
    world.insert_resource(EguiRenderPhase {
        state: initial_state,
        paint_jobs: Vec::new(),
    });
    world.insert_resource(desc);
    world.insert_resource(EguiWinitPlatform(platform));
    world.insert_resource(egui::Context::default())
}

fn begin_frame(
    ctx: Res<egui::Context>,
    mut winit_state: ResMut<EguiWinitPlatform>,
    windows: Res<Windows>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    let window = windows.primary();
    let winit_window = winit_windows
        .get_window(window.id())
        .expect("winit window not found");
    ctx.begin_frame(winit_state.0.take_egui_input(winit_window));
}

fn hello(ctx: Res<egui::Context>) {
    egui::Window::new("Hello title")
        .resizable(true)
        .collapsible(true)
        .show(&ctx, |ui| {
            ui.label("Hello label");
            if ui.button("test").clicked() {
                log::info!("click");
            }
        });
}

fn handle_mouse_events(
    mut mouse_button_input_events: EventReader<MouseButtonInput>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut platform: ResMut<EguiWinitPlatform>,
    ctx: ResMut<egui::Context>,
    windows: Res<Windows>,
) {
    for ev in cursor_moved_events.iter() {
        platform.0.on_event(
            &ctx,
            &winit::event::WindowEvent::CursorMoved {
                device_id: unsafe { DeviceId::dummy() },
                modifiers: ModifiersState::empty(),
                position: winit::dpi::PhysicalPosition {
                    x: ev.position.x as f64,
                    y: (windows.primary().physical_height() - ev.position.y as u32) as f64,
                },
            },
        );
    }

    for ev in mouse_button_input_events.iter() {
        platform.0.on_event(
            &ctx,
            &winit::event::WindowEvent::MouseInput {
                device_id: unsafe { DeviceId::dummy() },
                modifiers: ModifiersState::empty(),
                state: match ev.state {
                    bevy::input::ButtonState::Pressed => winit::event::ElementState::Pressed,
                    bevy::input::ButtonState::Released => winit::event::ElementState::Released,
                },
                button: match ev.button {
                    MouseButton::Left => winit::event::MouseButton::Left,
                    MouseButton::Right => winit::event::MouseButton::Right,
                    MouseButton::Middle => winit::event::MouseButton::Middle,
                    MouseButton::Other(x) => winit::event::MouseButton::Other(x),
                },
            },
        );
    }
}

impl<'w> RenderPhase for EguiRenderPhase<'w> {
    #[allow(clippy::type_complexity)]
    fn update(&mut self, world: &mut World) {
        let (
            renderer,
            screen_desc,
            egui_ctx,
            mut render_pass,
            mut platform,
            windows,
            winit_windows,
        ) = self.state.get_mut(world);

        let egui::FullOutput {
            shapes,
            textures_delta,
            platform_output,
            ..
        } = egui_ctx.end_frame();
        self.paint_jobs = egui_ctx.tessellate(shapes);
        let window = winit_windows
            .get_window(windows.primary().id())
            .expect("Failed to get primary window");
        platform
            .0
            .handle_platform_output(window, &egui_ctx, platform_output);

        for (id, image_delta) in textures_delta.set {
            render_pass.update_texture(&renderer.device, &renderer.queue, id, &image_delta);
        }

        render_pass.update_buffers(
            &renderer.device,
            &renderer.queue,
            &self.paint_jobs,
            &screen_desc,
        );
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let desc = world.resource::<egui_wgpu::renderer::ScreenDescriptor>();
        let render_pass = world.non_send_resource::<egui_wgpu::renderer::RenderPass>();

        render_pass.execute(encoder, view, &self.paint_jobs, desc, None)
    }
}
