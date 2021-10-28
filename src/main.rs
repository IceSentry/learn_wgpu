use bevy::{
    input::InputPlugin,
    prelude::*,
    window::{WindowPlugin, WindowResized},
    winit::{WinitPlugin, WinitWindows},
};
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Warn)
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
        .add_system(title_diagnostic)
        .add_system(bevy::input::system::exit_on_esc_system)
        .run();
}

struct Pipeline {
    wgpu_pipeline: wgpu::RenderPipeline,
    buffers: Buffers,
}

struct Buffers {
    vertex_buffer: wgpu::Buffer,
    num_vertices: usize,
    index_buffer: Option<wgpu::Buffer>,
    num_indices: usize,
}

struct Pipelines {
    pipelines: Vec<Pipeline>,
    selected_pipeline_index: usize,
}

impl Pipelines {
    fn get_selected_pipeline(&self) -> &Pipeline {
        &self.pipelines[self.selected_pipeline_index]
    }
}

fn setup(mut commands: Commands, winit_windows: Res<WinitWindows>, windows: Res<Windows>) {
    if let Some(window) = windows.get_primary() {
        let window_handle = winit_windows
            .get_window(window.id())
            .expect("winit window not found");

        let mut renderer = futures::executor::block_on(WgpuRenderer::new(window_handle));

        let triangle_pipeline = Pipeline {
            wgpu_pipeline: renderer.create_wgpu_render_pipeline(include_str!("shader.wgsl")),
            buffers: renderer.create_buffers(TRIANGLE_VERTICES, None),
        };

        let pentagon_pipeline = Pipeline {
            wgpu_pipeline: renderer.create_wgpu_render_pipeline(include_str!("shader.wgsl")),
            buffers: renderer.create_buffers(PENTAGON_VERTICES, Some(PENTAGON_INDICES)),
        };

        commands.insert_resource(renderer);
        commands.insert_resource(Pipelines {
            pipelines: vec![triangle_pipeline, pentagon_pipeline],
            selected_pipeline_index: 0,
        });
    }
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

fn title_diagnostic(time: Res<Time>, mut windows: ResMut<Windows>) {
    let window = windows.get_primary_mut().unwrap();
    window.set_title(format!("dt: {}ms", time.delta().as_millis()));
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

const TRIANGLE_VERTICES: &[Vertex] = &[
    Vertex {
        position: [0.0, 0.5, 0.0],
        color: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0],
        color: [0.0, 0.0, 1.0],
    },
];

const PENTAGON_VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        color: [0.5, 0.0, 0.5],
    },
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        color: [0.5, 0.0, 0.5],
    },
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        color: [0.5, 0.0, 0.5],
    },
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        color: [0.5, 0.0, 0.5],
    },
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        color: [0.5, 0.0, 0.5],
    },
];

const PENTAGON_INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4, 0];

struct WgpuRenderer {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    clear_color: wgpu::Color,
}

impl WgpuRenderer {
    // Creating some of the wgpu types requires async code
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to request adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None, // Trace path
            )
            .await
            .expect("Failed to request device");

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            clear_color: wgpu::Color::BLACK,
        }
    }

    fn create_wgpu_render_pipeline(&mut self, shader_string: &str) -> wgpu::RenderPipeline {
        let shader = self
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_string.into()),
            });

        let render_piepline_layout =
            self.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });

        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_piepline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "main",
                    buffers: &[Vertex::layout()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "main",
                    targets: &[wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    clamp_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
            })
    }

    fn create_buffers(&mut self, vertices: &[Vertex], indices: Option<&[u16]>) -> Buffers {
        Buffers {
            vertex_buffer: self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
            num_vertices: vertices.len(),
            index_buffer: indices.map(|indices| {
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Index Buffer"),
                        contents: bytemuck::cast_slice(indices),
                        usage: wgpu::BufferUsages::INDEX,
                    })
            }),
            num_indices: indices.map(|indices| indices.len()).unwrap_or(0),
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            log::info!("resizing");
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn render(&mut self, render_pipeline: &Pipeline) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            render_pass.set_pipeline(&render_pipeline.wgpu_pipeline);
            render_pass.set_vertex_buffer(0, render_pipeline.buffers.vertex_buffer.slice(..));

            if let Some(index_buffer) = &render_pipeline.buffers.index_buffer {
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..render_pipeline.buffers.num_indices as u32, 0, 0..1);
            } else {
                render_pass.draw(0..render_pipeline.buffers.num_vertices as u32, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        output.present();

        Ok(())
    }
}
