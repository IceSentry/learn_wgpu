use bevy::{
    input::InputPlugin,
    math::{const_vec3, vec3},
    prelude::*,
    window::{WindowPlugin, WindowResized},
    winit::{WinitPlugin, WinitWindows},
};
use camera::{Camera, CameraController, CameraUniform};
use depth_pass::DepthPass;
use renderer::{Instance, Pipeline, Vertex, WgpuRenderer};
use texture::Texture;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

mod camera;
mod depth_pass;
mod renderer;
mod texture;

// const TRIANGLE_VERTICES: &[Vertex] = &[
//     Vertex {
//         position: [0.0, 0.5, 0.0],
//         color: [1.0, 0.0, 0.0],
//         uv: [0.0, 0.0],
//     },
//     Vertex {
//         position: [-0.5, -0.5, 0.0],
//         color: [0.0, 1.0, 0.0],
//         uv: [0.0, 0.0],
//     },
//     Vertex {
//         position: [0.5, -0.5, 0.0],
//         color: [0.0, 0.0, 1.0],
//         uv: [0.0, 0.0],
//     },
// ];

// const PENTAGON_VERTICES: &[Vertex] = &[
//     Vertex {
//         position: [-0.0868241, 0.49240386, 0.0],
//         color: [0.5, 0.0, 0.5],
//         uv: [0.0, 0.0],
//     },
//     Vertex {
//         position: [-0.49513406, 0.06958647, 0.0],
//         color: [0.5, 0.0, 0.5],
//         uv: [0.0, 0.0],
//     },
//     Vertex {
//         position: [-0.21918549, -0.44939706, 0.0],
//         color: [0.5, 0.0, 0.5],
//         uv: [0.0, 0.0],
//     },
//     Vertex {
//         position: [0.35966998, -0.3473291, 0.0],
//         color: [0.5, 0.0, 0.5],
//         uv: [0.0, 0.0],
//     },
//     Vertex {
//         position: [0.44147372, 0.2347359, 0.0],
//         color: [0.5, 0.0, 0.5],
//         uv: [0.0, 0.0],
//     },
// ];

// const PENTAGON_INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4, 0];

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        color: [1.0, 1.0, 1.0],
        uv: [0.4131759, 0.00759614],
    },
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        color: [1.0, 1.0, 1.0],
        uv: [0.0048659444, 0.43041354],
    },
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        color: [1.0, 1.0, 1.0],
        uv: [0.28081453, 0.949397],
    },
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        color: [1.0, 1.0, 1.0],
        uv: [0.85967, 0.84732914],
    },
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        color: [1.0, 1.0, 1.0],
        uv: [0.9414737, 0.2652641],
    },
];

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4, 0];

const NUM_INSTANCES_PER_ROW: u32 = 10;
const INSTANCE_DISPLACEMENT: Vec3 = const_vec3!([
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
    0.0,
    NUM_INSTANCES_PER_ROW as f32 * 0.5,
]);

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .init();

    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugin(WindowPlugin::default())
        .add_plugin(WinitPlugin)
        .add_plugin(InputPlugin::default())
        .add_startup_system(setup)
        .add_system(resize)
        .add_system(render)
        .add_system(cursor_moved)
        .add_system(update_window_title)
        .add_system(update_camera)
        .add_system(move_instances)
        .add_system(bevy::input::system::exit_on_esc_system)
        .run();
}

struct CameraBuffer(wgpu::Buffer);
struct Instances(Vec<Instance>);

fn setup(mut commands: Commands, winit_windows: NonSendMut<WinitWindows>, windows: Res<Windows>) {
    let bevy_window = windows.get_primary().expect("bevy window not found");
    let winit_window = winit_windows
        .get_window(bevy_window.id())
        .expect("winit window not found");

    let mut renderer = futures::executor::block_on(WgpuRenderer::new(winit_window));

    let texture = Texture::from_bytes(
        &renderer,
        include_bytes!("assets/happy-tree.png"),
        "happy-tree.png",
    )
    .expect("failed to create texture");

    let width = renderer.config.width as f32;
    let height = renderer.config.height as f32;
    let camera = Camera {
        eye: vec3(0.0, 1.0, 2.0),
        target: vec3(0.0, 0.0, 0.0),
        up: Vec3::Y,
        aspect: width / height,
        fov_y: 45.0,
        z_near: 0.1,
        z_far: 100.0,
    };

    let mut camera_uniform = CameraUniform::new();
    camera_uniform.update_view_proj(&camera);

    let camera_buffer = renderer
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    let mut instances: Vec<_> = vec![];
    for z in 0..NUM_INSTANCES_PER_ROW {
        for x in 0..NUM_INSTANCES_PER_ROW {
            let translation = vec3(x as f32, 0.0, z as f32) - INSTANCE_DISPLACEMENT;
            let rotation = if translation == Vec3::ZERO {
                Quat::from_axis_angle(Vec3::Z, 0.0)
            } else {
                Quat::from_axis_angle(translation.normalize(), std::f32::consts::FRAC_PI_4)
            };
            instances.push(Instance {
                translation,
                rotation,
            });
        }
    }

    let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();

    let pipeline = renderer.create_pipeline(
        include_str!("shader.wgsl"),
        VERTICES,
        INDICES,
        &texture,
        &camera_buffer,
        &instance_data,
    );

    let depth_pass = DepthPass::new(&renderer.device, &renderer.config);

    commands.insert_resource(renderer);
    commands.insert_resource(pipeline);
    commands.insert_resource(camera);
    commands.insert_resource(CameraController::new(0.05));
    commands.insert_resource(camera_uniform);
    commands.insert_resource(CameraBuffer(camera_buffer));
    commands.insert_resource(Instances(instances));
    commands.insert_resource(depth_pass);
}

fn resize(
    mut renderer: ResMut<WgpuRenderer>,
    mut events: EventReader<WindowResized>,
    windows: Res<Windows>,
    mut depth_pass: ResMut<DepthPass>,
) {
    for event in events.iter() {
        let window = windows.get(event.id).expect("window not found");
        renderer.resize(PhysicalSize {
            width: window.physical_width(),
            height: window.physical_height(),
        });
        depth_pass.resize(&renderer.device, &renderer.config);
    }
}

fn render(
    mut renderer: ResMut<WgpuRenderer>,
    pipeline: Res<Pipeline>,
    instances: Res<Instances>,
    depth_pass: Res<DepthPass>,
) {
    match renderer.render(&pipeline, &instances.0, &depth_pass) {
        Ok(_) => {}
        Err(e) => log::error!("{:?}", e),
    }
}

fn cursor_moved(mut renderer: ResMut<WgpuRenderer>, mut events: EventReader<CursorMoved>) {
    for event in events.iter() {
        renderer.clear_color = wgpu::Color {
            r: event.position.x as f64 / renderer.size.width as f64,
            g: event.position.y as f64 / renderer.size.height as f64,
            ..renderer.clear_color
        };
    }
}

fn update_window_title(time: Res<Time>, mut windows: ResMut<Windows>) {
    let window = windows.get_primary_mut().unwrap();
    window.set_title(format!("dt: {}ms", time.delta().as_millis()));
}

fn update_camera(
    mut camera_controller: ResMut<CameraController>,
    keyboard_input: Res<Input<KeyCode>>,
    renderer: Res<WgpuRenderer>,
    mut camera: ResMut<Camera>,
    mut camera_uniform: ResMut<CameraUniform>,
    camera_buffer: Res<CameraBuffer>,
) {
    camera_controller.is_forward_pressed = keyboard_input.pressed(KeyCode::W);
    camera_controller.is_left_pressed = keyboard_input.pressed(KeyCode::A);
    camera_controller.is_backward_pressed = keyboard_input.pressed(KeyCode::S);
    camera_controller.is_right_pressed = keyboard_input.pressed(KeyCode::D);

    camera_controller.update_camera(&mut camera);

    camera_uniform.update_view_proj(&camera);

    renderer.queue.write_buffer(
        &camera_buffer.0,
        0,
        bytemuck::cast_slice(&[*camera_uniform]),
    );
}

fn move_instances(
    renderer: Res<WgpuRenderer>,
    pipeline: Res<Pipeline>,
    mut instances: ResMut<Instances>,
    time: Res<Time>,
    mut wave: Local<Wave>,
) {
    wave.offset += time.delta_seconds() * wave.frequency;

    for instance in instances.0.iter_mut() {
        instance.translation.y = wave.wave_height(instance.translation.x, instance.translation.z);
    }

    let data: Vec<_> = instances.0.iter().map(Instance::to_raw).collect();
    renderer.queue.write_buffer(
        &pipeline.buffers.instance_buffer,
        0,
        bytemuck::cast_slice(&data[..]),
    );
}

pub struct Wave {
    pub amplitude: f32,
    pub wavelength: f32,
    pub frequency: f32,
    pub offset: f32,
}

impl Default for Wave {
    fn default() -> Self {
        Self {
            amplitude: 0.25,
            wavelength: 6.0,
            frequency: 3.0,
            offset: 0.0,
        }
    }
}

impl Wave {
    pub fn wave_height(&self, x: f32, z: f32) -> f32 {
        // Wave number
        let k = std::f32::consts::TAU / self.wavelength;
        let r = (x * x + z * z).sqrt();
        self.amplitude * (k * (r - self.offset)).sin()
    }
}
