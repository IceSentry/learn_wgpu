use bevy::{
    input::InputPlugin,
    prelude::*,
    window::{WindowPlugin, WindowResized},
    winit::{WinitPlugin, WinitWindows},
};
use renderer::{Pipeline, Vertex, WgpuRenderer};
use texture::Texture;
use winit::dpi::PhysicalSize;

mod renderer;
mod texture;

const TRIANGLE_VERTICES: &[Vertex] = &[
    Vertex {
        position: [0.0, 0.5, 0.0],
        color: [1.0, 0.0, 0.0],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0],
        color: [0.0, 0.0, 1.0],
        uv: [0.0, 0.0],
    },
];

const PENTAGON_VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        color: [0.5, 0.0, 0.5],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        color: [0.5, 0.0, 0.5],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        color: [0.5, 0.0, 0.5],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        color: [0.5, 0.0, 0.5],
        uv: [0.0, 0.0],
    },
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        color: [0.5, 0.0, 0.5],
        uv: [0.0, 0.0],
    },
];

const PENTAGON_INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4, 0];

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

struct Pipelines {
    pipelines: Vec<Pipeline>,
    selected_pipeline_index: usize,
}

impl Pipelines {
    fn get_selected_pipeline(&self) -> &Pipeline {
        &self.pipelines[self.selected_pipeline_index]
    }
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
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
        .add_system(spacebar)
        .add_system(update_window_title)
        .add_system(bevy::input::system::exit_on_esc_system)
        .run();
}

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

    let triangle_pipeline = renderer.create_pipeline(
        include_str!("shader.wgsl"),
        TRIANGLE_VERTICES,
        None,
        &texture,
    );
    let pentagon_pipeline = renderer.create_pipeline(
        include_str!("shader.wgsl"),
        PENTAGON_VERTICES,
        Some(PENTAGON_INDICES),
        &texture,
    );

    let pipe = renderer.create_pipeline(
        include_str!("shader.wgsl"),
        VERTICES,
        Some(PENTAGON_INDICES),
        &texture,
    );

    commands.insert_resource(renderer);
    commands.insert_resource(Pipelines {
        pipelines: vec![pipe],
        selected_pipeline_index: 0,
    });
}

fn resize(
    mut renderer: ResMut<WgpuRenderer>,
    mut events: EventReader<WindowResized>,
    windows: Res<Windows>,
) {
    for event in events.iter() {
        let window = windows.get(event.id).expect("window not found");
        renderer.resize(PhysicalSize {
            width: window.physical_width(),
            height: window.physical_height(),
        });
    }
}

fn render(mut renderer: ResMut<WgpuRenderer>, pipelines: Res<Pipelines>) {
    match renderer.render(pipelines.get_selected_pipeline()) {
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

fn spacebar(keyboard_input: Res<Input<KeyCode>>, mut pipelines: ResMut<Pipelines>) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        pipelines.selected_pipeline_index = (pipelines.selected_pipeline_index + 1) % 2;
        log::info!(
            "selected_pipeline_index {}",
            pipelines.selected_pipeline_index
        )
    }
}

fn update_window_title(time: Res<Time>, mut windows: ResMut<Windows>) {
    let window = windows.get_primary_mut().unwrap();
    window.set_title(format!("dt: {}ms", time.delta().as_millis()));
}
