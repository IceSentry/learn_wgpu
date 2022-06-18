use bevy::prelude::{Added, Changed, Commands, Component, Entity, Query, Res, With, Without};
use wgpu::util::DeviceExt;

use crate::renderer::WgpuRenderer;
use crate::{model::Model, transform::Transform};

#[derive(Component)]
pub struct InstanceBuffer(pub wgpu::Buffer);

/// If you want to spawn multiple instances of the same mesh you need to
/// specify the Transform of each instance in this component.
/// If the renderer sees this component it will draw it using draw_instanced
#[derive(Component)]
pub struct Instances(pub Vec<Transform>);

/// Creates the necessary IntanceBuffer on any Model created with a Model and a Transform or Instances
#[allow(clippy::type_complexity)]
pub fn create_instance_buffer(
    mut commands: Commands,
    renderer: Res<WgpuRenderer>,
    instanced_query: Query<
        (Entity, &Instances),
        (
            Added<Instances>,
            With<Model>,
            Without<Transform>,
            Without<InstanceBuffer>,
        ),
    >,
    query: Query<
        (Entity, &Transform),
        (
            Added<Transform>,
            With<Model>,
            Without<Instances>,
            Without<InstanceBuffer>,
        ),
    >,
) {
    for (entity, instances) in instanced_query.iter() {
        let instance_data: Vec<_> = instances.0.iter().map(Transform::to_raw).collect();
        let instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instance_data),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        commands
            .entity(entity)
            .insert(InstanceBuffer(instance_buffer));
    }

    for (entity, transform) in query.iter() {
        log::info!("create instance buffer for single mesh");
        let instance_buffer =
            renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&[transform.to_raw()]),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        commands
            .entity(entity)
            .insert(InstanceBuffer(instance_buffer));
    }
}

pub fn update_instance_buffer(
    renderer: Res<WgpuRenderer>,
    query: Query<(&InstanceBuffer, &Instances), Changed<Instances>>,
) {
    for (buffer, instances) in query.iter() {
        let data: Vec<_> = instances.0.iter().map(Transform::to_raw).collect();
        renderer
            .queue
            .write_buffer(&buffer.0, 0, bytemuck::cast_slice(&data[..]));
    }
}
