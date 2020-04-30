use crate::Config;

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
    color: [f32; 3],
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
                    format: wgpu::VertexFormat::Float3,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        color: [0.5, 0.0, 0.5],
    }, // A
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        color: [0.5, 0.0, 0.5],
    }, // B
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        color: [0.5, 0.0, 0.5],
    }, // C
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        color: [0.5, 0.0, 0.5],
    }, // D
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        color: [0.5, 0.0, 0.5],
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

pub struct Renderer {
    pub surface: wgpu::Surface,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub sc_desc: wgpu::SwapChainDescriptor,
    pub swap_chain: wgpu::SwapChain,
    pub render_pipeline: wgpu::RenderPipeline,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub render_format: wgpu::TextureFormat,
    pub scale_factor: f64,
    pub clear_color: wgpu::Color,
    pub last_frame: Instant,
    pub last_frame_duration: Duration,
    pub imgui_renderer: imgui_wgpu::Renderer,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
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
            present_mode: wgpu::PresentMode::Mailbox,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        // load texture
        {
            let diffuse_bytes = include_bytes!("assets/happy-tree.png");
            let diffuse_image = image::load_from_memory(diffuse_bytes).unwrap();
            let diffuse_rgba = diffuse_image.as_rgba8().unwrap();

            use image::GenericImageView;
            let dimensions = diffuse_image.dimensions();

            let size = wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth: 1,
            };
            let diffuse_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("happy-tree"),
                // All textures are stored as 3d, we represent our 2d texture
                // by setting depth to 1.
                size: wgpu::Extent3d {
                    width: dimensions.0,
                    height: dimensions.1,
                    depth: 1,
                },
                array_layer_count: 1,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                // SAMPLED tells wgpu that we want to use this texture in shaders
                // COPY_DST means that we want to copy data to this texture
                usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
            });

            let buffer = device.create_buffer_with_data(&diffuse_rgba, wgpu::BufferUsage::COPY_SRC);
        }

        let render_pipeline = {
            let vert_shader = include_str!("shader.vert");
            let frag_shader = include_str!("shader.frag");

            let vs_spirv = glsl_to_spirv::compile(vert_shader, glsl_to_spirv::ShaderType::Vertex)
                .expect("failed to compile vertex shader");
            let fs_spirv = glsl_to_spirv::compile(frag_shader, glsl_to_spirv::ShaderType::Fragment)
                .expect("failed to compile frag shader");

            let vs_data = wgpu::read_spirv(vs_spirv).expect("failed to read vertex shader");
            let fs_data = wgpu::read_spirv(fs_spirv).expect("failed to read frag shader");

            let vs_module = device.create_shader_module(&vs_data);
            let fs_module = device.create_shader_module(&fs_data);

            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    bind_group_layouts: &[],
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
