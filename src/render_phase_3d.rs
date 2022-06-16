use crate::{
    camera,
    depth_pass::DepthPass,
    instances::InstanceBuffer,
    light::{draw_light_model, Light},
    material, mesh,
    model::{self, Model, ModelVertex},
    renderer::{RenderPhase, WgpuRenderer},
    texture::{self, Texture},
    transform::TransformRaw,
    Instances, ShowDepthBuffer,
};
use bevy::prelude::{Color, Component, QueryState, With, Without, World};
use wgpu::CommandEncoder;

#[derive(Component)]
pub struct LightBindGroup(pub wgpu::BindGroup);

pub struct DepthTexture(pub Texture);

pub struct ClearColor(pub Color);

pub struct CameraBindGroup(pub wgpu::BindGroup);

// TODO not sure if I need a concept of RenderPhase I can probably get away
// with all of this on the renderer as long as I encapsulate render passes
#[allow(clippy::type_complexity)]
pub struct RenderPhase3d {
    pub opaque_pass: OpaquePass,
}

impl RenderPhase3d {
    pub fn from_world(world: &mut World) -> Self {
        Self {
            opaque_pass: OpaquePass::from_world(world),
        }
    }
}

impl RenderPhase for RenderPhase3d {
    fn update<'w>(&'w mut self, world: &'w mut World) {
        self.opaque_pass.update(world);
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut CommandEncoder) {
        // TODO the RenderPhase3d should probably own this
        let depth_pass = world.resource::<DepthPass>();

        self.opaque_pass.render(world, view, encoder);

        if world.resource::<ShowDepthBuffer>().0 {
            depth_pass.render(view, encoder);
        }
    }
}

#[derive(Component)]
pub struct Transparent;

// TODO pass could own a pipeline
#[allow(clippy::type_complexity)]
pub struct OpaquePass {
    pub render_pipeline: wgpu::RenderPipeline,
    pub light_render_pipeline: wgpu::RenderPipeline,
    pub transparent_render_pipeline: wgpu::RenderPipeline,
    pub light_query: QueryState<&'static Model, With<Light>>,
    pub model_query: QueryState<
        (
            &'static Model,
            &'static InstanceBuffer,
            Option<&'static Instances>,
        ),
        (Without<Light>, Without<Transparent>),
    >,
    pub transparent_model_query: QueryState<
        (
            &'static Model,
            &'static InstanceBuffer,
            Option<&'static Instances>,
        ),
        (Without<Light>, With<Transparent>),
    >,
}

impl OpaquePass {
    pub fn from_world(world: &mut World) -> Self {
        let renderer = world.resource::<WgpuRenderer>();

        let render_pipeline_layout = {
            // TODO create_mesh_view_bind_group
            // let mesh_view_layout =
            //     renderer
            //         .device
            //         .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            //             label: Some("mesh_view_bind_group_layout"),
            //             entries: &[
            //                 // Camera
            //                 camera::bind_group_layout_entry(0),
            //                 // Light
            //                 Light::bind_group_layout_entry(1),
            //             ],
            //         });
            // TODO consider storing layouts in ECS
            // let material_layout = material::bind_group_layout(&renderer.device);
            // let mesh_layout = mesh::bind_group_layout(&renderer.device);
            // renderer
            //     .device
            //     .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            //         label: Some("Render Pipeline Layout"),
            //         bind_group_layouts: &[&mesh_view_layout, &material_layout, &mesh_layout],
            //         push_constant_ranges: &[],
            //     })
        };

        let render_pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[
                        &camera::bind_group_layout(&renderer.device),
                        &Light::bind_group_layout(&renderer.device),
                        &texture::bind_group_layout(&renderer.device),
                    ],
                    push_constant_ranges: &[],
                });

        // TODO have a better way to attach draw commands to a pipeline
        let render_pipeline = renderer.create_render_pipeline(
            "Opaque Render Pipeline",
            include_str!("shaders/shader.wgsl"),
            &render_pipeline_layout,
            &[model::ModelVertex::layout(), TransformRaw::layout()],
            Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            wgpu::BlendState::REPLACE,
        );

        let transparent_render_pipeline = renderer.create_render_pipeline(
            "Transparent Render Pipeline",
            include_str!("shaders/shader.wgsl"),
            &render_pipeline_layout,
            &[model::ModelVertex::layout(), TransformRaw::layout()],
            Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            wgpu::BlendState::ALPHA_BLENDING,
        );

        let light_render_pipeline = renderer.create_render_pipeline(
            "Light Render Pipeline",
            include_str!("shaders/light.wgsl"),
            &renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Light Pipeline Layout"),
                    bind_group_layouts: &[
                        &camera::bind_group_layout(&renderer.device),
                        &Light::bind_group_layout(&renderer.device),
                    ],
                    push_constant_ranges: &[],
                }),
            &[ModelVertex::layout()],
            Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            wgpu::BlendState::REPLACE,
        );

        Self {
            render_pipeline,
            light_render_pipeline,
            transparent_render_pipeline,
            light_query: world.query_filtered(),
            model_query: world.query_filtered(),
            transparent_model_query: world.query_filtered(),
        }
    }

    pub fn update<'w>(&'w mut self, world: &'w mut World) {
        self.light_query.update_archetypes(world);
        self.model_query.update_archetypes(world);
        self.transparent_model_query.update_archetypes(world);
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let camera_bind_group = world.resource::<CameraBindGroup>();
        let light_bind_group = world.resource::<LightBindGroup>();
        let depth_texture = world.resource::<DepthTexture>();
        let clear_color = world.resource::<ClearColor>();

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Opaque Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: clear_color.0.r() as f64,
                        g: clear_color.0.g() as f64,
                        b: clear_color.0.b() as f64,
                        a: clear_color.0.a() as f64,
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_texture.0.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        // TODO figure out how to sort models
        render_pass.set_pipeline(&self.render_pipeline);
        for (model, instance_buffer, instances) in self.model_query.iter_manual(world) {
            // The draw function also uses the instance buffer under the hood it simply is of size 1
            render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));
            let transparent = false;
            if let Some(instances) = instances {
                model.draw_instanced(
                    &mut render_pass,
                    0..instances.0.len() as u32,
                    &camera_bind_group.0,
                    &light_bind_group.0,
                    transparent,
                );
            } else {
                model.draw(
                    &mut render_pass,
                    &camera_bind_group.0,
                    &light_bind_group.0,
                    transparent,
                );
            }
        }

        // TODO I need a better way to identify transparent meshes in a model
        render_pass.set_pipeline(&self.transparent_render_pipeline);
        for (model, instance_buffer, instances) in self.model_query.iter_manual(world) {
            // The draw function also uses the instance buffer under the hood it simply is of size 1
            render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));
            let transparent = true;
            if let Some(instances) = instances {
                model.draw_instanced(
                    &mut render_pass,
                    0..instances.0.len() as u32,
                    &camera_bind_group.0,
                    &light_bind_group.0,
                    transparent,
                );
            } else {
                model.draw(
                    &mut render_pass,
                    &camera_bind_group.0,
                    &light_bind_group.0,
                    transparent,
                );
            }
        }

        render_pass.set_pipeline(&self.light_render_pipeline);
        for light_model in self.light_query.iter_manual(world) {
            draw_light_model(
                &mut render_pass,
                light_model,
                &camera_bind_group.0,
                &light_bind_group.0,
            );
        }
    }
}
