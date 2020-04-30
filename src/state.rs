use std::time::Instant;
use wgpu_glyph::{GlyphBrushBuilder, Section};
use winit::{event::*, window::Window};

pub struct State {
    pub surface: wgpu::Surface,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub sc_desc: wgpu::SwapChainDescriptor,
    pub swap_chain: wgpu::SwapChain,
    pub render_pipelines: Vec<wgpu::RenderPipeline>,
    pub render_pipeline_index: usize,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub render_format: wgpu::TextureFormat,
    pub scale_factor: f64,
    pub clear_color: wgpu::Color,
    pub last_frame: Instant,
    pub demo_open: bool,
}

impl State {
    pub async fn new(window: &Window) -> Self {
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

        let (device, queue) = adapter
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

        let render_pipeline = State::init_simple_render_pipeline(
            &device,
            &sc_desc,
            include_str!("shader.vert"),
            include_str!("shader.frag"),
        );

        let render_pipeline_2 = State::init_simple_render_pipeline(
            &device,
            &sc_desc,
            include_str!("shader2.vert"),
            include_str!("shader2.frag"),
        );

        let clear_color = wgpu::Color::default();
        let scale_factor = 1.0;

        Self {
            surface,
            adapter,
            device,
            queue,
            sc_desc,
            swap_chain,
            render_pipelines: vec![render_pipeline, render_pipeline_2],
            render_pipeline_index: 0,
            size,
            scale_factor,
            clear_color,
            last_frame: Instant::now(),
            demo_open: true,
            render_format,
        }
    }

    fn init_simple_render_pipeline(
        device: &wgpu::Device,
        sc_desc: &wgpu::SwapChainDescriptor,
        vert_shader: &str,
        frag_shader: &str,
    ) -> wgpu::RenderPipeline {
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
                vertex_buffers: &[],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let center_w = (self.size.width / 2) as f64;
                let center_h = (self.size.height / 2) as f64;
                let max_dist_to_center = (center_w.powi(2) + center_h.powi(2)).sqrt();
                let dist_to_center_normalized =
                    ((center_w - position.x).powi(2) + (center_h - position.y).powi(2)).sqrt()
                        / max_dist_to_center;
                self.clear_color = wgpu::Color {
                    r: dist_to_center_normalized,
                    g: 1.0 - dist_to_center_normalized,
                    b: 0.0,
                    a: 1.0,
                }
            }
            WindowEvent::KeyboardInput { input, .. } => {
                if let KeyboardInput {
                    state: ElementState::Pressed,
                    virtual_keycode: Some(VirtualKeyCode::Space),
                    ..
                } = input
                {
                    self.render_pipeline_index = match self.render_pipeline_index {
                        0 => 1,
                        1 => 0,
                        _ => 0,
                    }
                }
            }
            _ => return false,
        }
        true
    }

    pub fn update(&mut self) {}

    pub fn render(&mut self, window: &winit::window::Window) {
        let delta_t = self.last_frame.elapsed();
        self.last_frame = Instant::now();

        let frame = self
            .swap_chain
            .get_next_texture()
            .expect("Failed to get texture");

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

            render_pass.set_pipeline(&self.render_pipelines[self.render_pipeline_index]);
            render_pass.draw(0..3, 0..1);
        }

        let font: &[u8] = include_bytes!("Inconsolata-Regular.ttf");
        let mut glyph_brush = GlyphBrushBuilder::using_font_bytes(font)
            .expect("Load font")
            .build(&self.device, self.render_format);

        glyph_brush.queue(Section {
            text: "Hello wgpu_glyph",
            screen_position: (0.0, 0.0),
            ..Section::default()
        });

        glyph_brush.queue(Section {
            text: &format!("Frametime: {:?}", delta_t),
            screen_position: (0.0, 20.0),
            ..Section::default()
        });

        glyph_brush
            .draw_queued(
                &self.device,
                &mut encoder,
                &frame.view,
                self.size.width,
                self.size.height,
            )
            .expect("Draw queued");

        self.queue.submit(&[encoder.finish()]);
    }
}
