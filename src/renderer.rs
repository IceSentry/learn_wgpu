use crate::{
    camera::{Camera, CameraController},
    texture, Config,
};

use std::mem;
use std::time::{Duration, Instant};

use imgui::{im_str, Condition, Context, FontSource, Ui};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use wgpu_glyph::{GlyphBrushBuilder, Section};
use winit::window::Window;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        wgpu::VertexBufferDescriptor {
            stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float3,
                },
                wgpu::VertexAttributeDescriptor {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float2,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        tex_coords: [0.4131759, 0.00759614],
    }, // A
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        tex_coords: [0.0048659444, 0.43041354],
    }, // B
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        tex_coords: [0.28081453, 0.949397057],
    }, // C
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        tex_coords: [0.85967, 0.84732911],
    }, // D
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        tex_coords: [0.9414737, 0.2652641],
    }, // E
];

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

pub struct ImguiState {
    pub context: Context,
    pub platform: WinitPlatform,
}

impl ImguiState {
    pub fn new(window: &winit::window::Window, scale_factor: f64) -> Self {
        let mut imgui = Context::create();

        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);
        imgui.set_ini_filename(None);

        let font_size = (13.0 * scale_factor) as f32;
        imgui.io_mut().font_global_scale = (1.0 / scale_factor) as f32;

        imgui.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(imgui::FontConfig {
                oversample_h: 1,
                pixel_snap_h: true,
                size_pixels: font_size,
                ..Default::default()
            }),
        }]);

        Self {
            context: imgui,
            platform,
        }
    }

    pub fn prepare(&mut self, window: &winit::window::Window, delta_t: Duration) -> Ui {
        self.platform
            .prepare_frame(self.context.io_mut(), &window)
            .expect("Failed to prepare frame");
        let ui = self.context.frame();

        {
            imgui::Window::new(im_str!("Debug info"))
                // .size([width, 100.0], Condition::FirstUseEver)
                .position([0.0, 0.0], Condition::FirstUseEver)
                .build(&ui, || {
                    ui.text(im_str!("Frametime: {:?}", delta_t));
                    ui.separator();
                    let mouse_pos = ui.io().mouse_pos;
                    ui.text(im_str!(
                        "Mouse Position: ({:.1},{:.1})",
                        mouse_pos[0],
                        mouse_pos[1]
                    ));
                });
        }

        self.platform.prepare_render(&ui, &window);
        ui
    }
}

#[repr(C)] // We need this for Rust to store our data correctly for the shaders
#[derive(Debug, Copy, Clone)] // This is so we can store this in a buffer
struct Uniforms {
    view_proj: cgmath::Matrix4<f32>,
}

unsafe impl bytemuck::Pod for Uniforms {}
unsafe impl bytemuck::Zeroable for Uniforms {}

impl Uniforms {
    fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity(),
        }
    }

    fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix();
    }
}

pub struct Renderer {
    surface: wgpu::Surface,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    sc_desc: wgpu::SwapChainDescriptor,
    swap_chain: wgpu::SwapChain,
    render_pipeline: wgpu::RenderPipeline,
    pub size: winit::dpi::PhysicalSize<u32>,
    render_format: wgpu::TextureFormat,
    scale_factor: f64,
    pub clear_color: wgpu::Color,
    pub last_frame: Instant,
    pub last_frame_duration: Duration,
    imgui_renderer: imgui_wgpu::Renderer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    diffuse_texture: texture::Texture,
    diffuse_bind_group: wgpu::BindGroup,
    camera: Camera,
    pub camera_controller: CameraController,
    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl Renderer {
    pub async fn new(window: &Window, imgui_context: &mut imgui::Context) -> Self {
        let size = window.inner_size();
        let surface = wgpu::Surface::create(window);
        let adapter = wgpu::Adapter::request(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
            },
            wgpu::BackendBit::PRIMARY, // Vulakn + Metal + DX12 + WebGPU
        )
        .await
        .expect("Failed to request adapter");

        let (device, mut queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                extensions: wgpu::Extensions {
                    anisotropic_filtering: false,
                },
                limits: Default::default(),
            })
            .await;

        let render_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            // We use wgpu::TextureFormat::Bgra8UnormSrgb because that's the format
            // that's guaranteed to be natively supported by the swapchains of all the APIs/platforms
            format: render_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        // load texture
        let diffuse_bytes = include_bytes!("assets/happy-tree.png");
        let (diffuse_texture, cmd_buffer) =
            texture::Texture::from_bytes(&device, diffuse_bytes).unwrap();
        queue.submit(&[cmd_buffer]);

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            multisampled: false,
                            dimension: wgpu::TextureViewDimension::D2,
                            component_type: wgpu::TextureComponentType::Uint,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let camera = Camera {
            // +z is out of the screen
            eye: (0.0, 1.0, 2.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: sc_desc.width as f32 / sc_desc.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };
        let camera_controller = CameraController::new(0.2);

        let mut uniforms = Uniforms::new();
        uniforms.update_view_proj(&camera);

        let uniform_buffer = device.create_buffer_with_data(
            bytemuck::cast_slice(&[uniforms]),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        );

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                }],
                label: Some("uniform_bind_group_layout"),
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &uniform_buffer,
                    range: 0..std::mem::size_of_val(&uniforms) as wgpu::BufferAddress,
                },
            }],
            label: Some("uniform_bind_group"),
        });

        let render_pipeline = {
            let vert_shader = include_str!("shader.vert");
            let frag_shader = include_str!("shader.frag");

            let vs_spirv = glsl_to_spirv::compile(vert_shader, glsl_to_spirv::ShaderType::Vertex)
                .unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    panic!("Failed to compile vert shader");
                });
            let fs_spirv = glsl_to_spirv::compile(frag_shader, glsl_to_spirv::ShaderType::Fragment)
                .unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    panic!("Failed to compile vert shader");
                });

            let vs_data = wgpu::read_spirv(vs_spirv).expect("failed to read vertex shader");
            let fs_data = wgpu::read_spirv(fs_spirv).expect("failed to read frag shader");

            let vs_module = device.create_shader_module(&vs_data);
            let fs_module = device.create_shader_module(&fs_data);

            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
                });

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                layout: &render_pipeline_layout,
                vertex_stage: wgpu::ProgrammableStageDescriptor {
                    module: &vs_module,
                    entry_point: "main",
                },
                fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                    module: &fs_module,
                    entry_point: "main",
                }),
                rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: wgpu::CullMode::Back,
                    depth_bias: 0,
                    depth_bias_slope_scale: 0.0,
                    depth_bias_clamp: 0.0,
                }),
                color_states: &[wgpu::ColorStateDescriptor {
                    format: sc_desc.format,
                    color_blend: wgpu::BlendDescriptor::REPLACE,
                    alpha_blend: wgpu::BlendDescriptor::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                }],
                primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                depth_stencil_state: None,
                vertex_state: wgpu::VertexStateDescriptor {
                    index_format: wgpu::IndexFormat::Uint16,
                    vertex_buffers: &[Vertex::desc()],
                },
                sample_count: 1,
                sample_mask: !0,
                alpha_to_coverage_enabled: false,
            })
        };

        let imgui_renderer =
            imgui_wgpu::Renderer::new(imgui_context, &device, &mut queue, sc_desc.format, None);

        // Setup buffers
        let vertex_buffer = device
            .create_buffer_with_data(bytemuck::cast_slice(VERTICES), wgpu::BufferUsage::VERTEX);
        let index_buffer =
            device.create_buffer_with_data(bytemuck::cast_slice(INDICES), wgpu::BufferUsage::INDEX);

        let clear_color = wgpu::Color::default();
        let scale_factor = 1.0;

        Self {
            surface,
            adapter,
            device,
            queue,
            sc_desc,
            swap_chain,
            render_pipeline,
            size,
            scale_factor,
            clear_color,
            last_frame: Instant::now(),
            last_frame_duration: Instant::now().elapsed(),
            render_format,
            imgui_renderer,
            vertex_buffer,
            index_buffer,
            num_indices: INDICES.len() as u32,
            diffuse_texture,
            diffuse_bind_group,
            camera,
            camera_controller,
            uniforms,
            uniform_bind_group,
            uniform_buffer,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>, scale_factor: Option<f64>) {
        if let Some(scale_factor) = scale_factor {
            self.scale_factor = scale_factor;
        }
        self.size = new_size;
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
    }

    pub fn update(&mut self, _delta_t: Duration) {
        self.camera_controller.update_camera(&mut self.camera);
        self.uniforms.update_view_proj(&self.camera);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("update encoder"),
            });

        let staging_buffer = self.device.create_buffer_with_data(
            bytemuck::cast_slice(&[self.uniforms]),
            wgpu::BufferUsage::COPY_SRC,
        );

        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &self.uniform_buffer,
            0,
            std::mem::size_of::<Uniforms>() as wgpu::BufferAddress,
        );

        self.queue.submit(&[encoder.finish()]);
    }

    pub fn render(&mut self, ui: imgui::Ui, delta_t: Duration, config: &Config) {
        let frame = match self.swap_chain.get_next_texture() {
            Ok(frame) => frame,
            Err(e) => {
                eprintln!("dropped frame: {:?}", e);
                return;
            }
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: self.clear_color,
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, &self.vertex_buffer, 0, 0);
            render_pass.set_index_buffer(&self.index_buffer, 0, 0);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        if config.debug.glyph {
            self.render_debug_text(&mut encoder, &frame, delta_t);
        }

        if config.debug.imgui {
            self.imgui_renderer
                .render(ui.render(), &self.device, &mut encoder, &frame.view)
                .expect("Imgui rendering failed");
        }

        self.queue.submit(&[encoder.finish()]);
    }

    fn render_debug_text(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::SwapChainOutput,
        delta_t: Duration,
    ) {
        let font: &[u8] = include_bytes!("assets/Inconsolata-Regular.ttf");
        let mut glyph_brush = GlyphBrushBuilder::using_font_bytes(font)
            .expect("Load font")
            .build(&self.device, self.render_format);

        glyph_brush.queue(Section {
            text: &format!("{:?}", delta_t),
            ..Section::default()
        });

        let curr_fps = 1.0 / delta_t.as_secs_f64();
        let last_fps = 1.0 / self.last_frame_duration.as_secs_f64();

        glyph_brush.queue(Section {
            text: &format!("{:.0}fps", last_fps * 0.9 + curr_fps * 0.1),
            screen_position: (0.0, 20.0),
            ..Section::default()
        });

        glyph_brush
            .draw_queued(
                &self.device,
                encoder,
                &frame.view,
                self.size.width,
                self.size.height,
            )
            .expect("Draw queued");
    }
}
