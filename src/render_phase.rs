use bevy::prelude::{Color, Component, QueryState, World};
use wgpu::CommandEncoder;

use crate::{
    depth_pass::DepthPass, light::draw_light_model, model::Model, renderer::Pipeline,
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
pub struct LightModel;

pub struct RenderPhase3d {
    // TODO this could just be a res
    pub clear_color: Color,
    pub model_query: QueryState<(
        &'static Model,
        Option<&'static InstanceCount>,
        Option<&'static LightModel>,
    )>,
    pub pipeline_query: QueryState<&'static Pipeline>,
}

impl RenderPhase for RenderPhase3d {
    fn update<'w>(&'w mut self, world: &'w mut World) {
        self.model_query.update_archetypes(world);
        self.pipeline_query.update_archetypes(world);
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut CommandEncoder) {
        let depth_pass = world.resource::<DepthPass>();

        // opaque phase
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear_color.r() as f64,
                            g: self.clear_color.g() as f64,
                            b: self.clear_color.b() as f64,
                            a: self.clear_color.a() as f64,
                        }),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_pass.texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            let pipeline = world.resource::<Pipeline>();
            let camera_bind_group = &pipeline.camera_bind_group;
            let light_bind_group = &pipeline.light_bind_group;

            for (model, instance_count, ligth_model) in self.model_query.iter_manual(world) {
                render_pass.set_vertex_buffer(1, pipeline.instance_buffer.slice(..));
                if ligth_model.is_some() {
                    render_pass.set_pipeline(&pipeline.light_pipeline);
                    draw_light_model(&mut render_pass, model, camera_bind_group, light_bind_group);
                }

                if let Some(InstanceCount(instance_count)) = instance_count {
                    render_pass.set_pipeline(&pipeline.render_pipeline);
                    model.draw_instanced(
                        &mut render_pass,
                        0..*instance_count as u32,
                        camera_bind_group,
                        light_bind_group,
                    );
                } else {
                    render_pass.set_pipeline(&pipeline.render_pipeline);
                    model.draw(&mut render_pass, camera_bind_group, light_bind_group);
                }
            }
        }

        if world.resource::<ShowDepthBuffer>().0 {
            depth_pass.render(view, encoder);
        }
    }
}
