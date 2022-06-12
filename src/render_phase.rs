use bevy::prelude::{Color, Component, QueryState, World};
use wgpu::CommandEncoder;

use crate::{
    depth_pass::DepthPass,
    light::{draw_light_model, Light},
    model::Model,
    renderer::Pipeline,
    texture::Texture,
    ShowDepthBuffer,
};

// NOTE: Is this trait necessary?
pub trait RenderPhase {
    fn update(&mut self, world: &mut World);
    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut CommandEncoder);
}

#[derive(Component)]
pub struct InstanceCount(pub usize);
#[derive(Component)]
pub struct InstanceBuffer(pub wgpu::Buffer);

#[derive(Component)]
pub struct LightBindGroup(pub wgpu::BindGroup);

pub struct DepthTexture(pub Texture);

pub struct ClearColor(pub Color);

pub struct CameraBindGroup(pub wgpu::BindGroup);

#[allow(clippy::type_complexity)]
pub struct RenderPhase3d {
    pub light_query: QueryState<&'static Model, bevy::prelude::With<Light>>,
    pub model_query: QueryState<(
        &'static Model,
        Option<&'static InstanceCount>,
        &'static InstanceBuffer,
    )>,
    pub pipeline_query: QueryState<&'static Pipeline>,
}

impl RenderPhase3d {
    pub fn from_world(world: &mut World) -> Self {
        Self {
            light_query: world.query_filtered(),
            model_query: world.query_filtered(),
            pipeline_query: world.query_filtered(),
        }
    }
}

impl RenderPhase for RenderPhase3d {
    fn update<'w>(&'w mut self, world: &'w mut World) {
        self.model_query.update_archetypes(world);
        self.light_query.update_archetypes(world);
        self.pipeline_query.update_archetypes(world);
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut CommandEncoder) {
        let depth_pass = world.resource::<DepthPass>();

        let pipeline = world.resource::<Pipeline>();
        let camera_bind_group = world.resource::<CameraBindGroup>();
        let light_bind_group = world.resource::<LightBindGroup>();
        let depth_texture = world.resource::<DepthTexture>();
        let clear_color = world.resource::<ClearColor>();

        // opaque phase
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
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

            for light_model in self.light_query.iter_manual(world) {
                render_pass.set_pipeline(&pipeline.light_pipeline);
                draw_light_model(
                    &mut render_pass,
                    light_model,
                    &camera_bind_group.0,
                    &light_bind_group.0,
                );
            }

            for (model, instance_count, instance_buffer) in self.model_query.iter_manual(world) {
                render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));

                if let Some(InstanceCount(instance_count)) = instance_count {
                    render_pass.set_pipeline(&pipeline.render_pipeline);
                    model.draw_instanced(
                        &mut render_pass,
                        0..*instance_count as u32,
                        &camera_bind_group.0,
                        &light_bind_group.0,
                    );
                } else {
                    render_pass.set_pipeline(&pipeline.render_pipeline);
                    model.draw(&mut render_pass, &camera_bind_group.0, &light_bind_group.0);
                }
            }
        }

        if world.resource::<ShowDepthBuffer>().0 {
            depth_pass.render(view, encoder);
        }
    }
}
