use crate::{
    depth_pass::DepthPass,
    instances::InstanceBuffer,
    light::{draw_light_model, Light},
    model::Model,
    renderer::{Pipeline, RenderPhase},
    texture::Texture,
    Instances, ShowDepthBuffer,
};
use bevy::prelude::{Color, Component, QueryState, With, Without, World};
use wgpu::CommandEncoder;

#[derive(Component)]
pub struct LightBindGroup(pub wgpu::BindGroup);

pub struct DepthTexture(pub Texture);

pub struct ClearColor(pub Color);

pub struct CameraBindGroup(pub wgpu::BindGroup);

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

pub struct OpaquePass {
    pub light_query: QueryState<&'static Model, With<Light>>,
    pub model_query: QueryState<
        (
            &'static Model,
            &'static InstanceBuffer,
            Option<&'static Instances>,
        ),
        Without<Light>,
    >,
    pub pipeline_query: QueryState<&'static Pipeline>,
}

impl OpaquePass {
    pub fn from_world(world: &mut World) -> Self {
        Self {
            light_query: world.query_filtered(),
            model_query: world.query_filtered(),
            pipeline_query: world.query_filtered(),
        }
    }

    pub fn update<'w>(&'w mut self, world: &'w mut World) {
        self.model_query.update_archetypes(world);
        self.light_query.update_archetypes(world);
        self.pipeline_query.update_archetypes(world);
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let pipeline = world.resource::<Pipeline>();
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

        for light_model in self.light_query.iter_manual(world) {
            render_pass.set_pipeline(&pipeline.light_pipeline);
            draw_light_model(
                &mut render_pass,
                light_model,
                &camera_bind_group.0,
                &light_bind_group.0,
            );
        }

        render_pass.set_pipeline(&pipeline.render_pipeline);
        for (model, instance_buffer, instances) in self.model_query.iter_manual(world) {
            // The draw function also uses the instance buffer under the hood it simply is of size 1
            render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));

            if let Some(instances) = instances {
                model.draw_instanced(
                    &mut render_pass,
                    0..instances.0.len() as u32,
                    &camera_bind_group.0,
                    &light_bind_group.0,
                );
            } else {
                model.draw(&mut render_pass, &camera_bind_group.0, &light_bind_group.0);
            }
        }
    }
}
