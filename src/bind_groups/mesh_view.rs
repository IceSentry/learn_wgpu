use bevy::prelude::*;
use wgpu::util::DeviceExt;

use crate::{camera::Camera, renderer::WgpuRenderer};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_position: [f32; 4],
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_position: [0.0; 4],
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_position = [camera.eye.x, camera.eye.y, camera.eye.z, 1.0];
        self.view_proj = camera.build_view_projection_matrix().to_cols_array_2d();
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Component)]
pub struct LightUniform {
    pub position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    pub color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}

impl LightUniform {
    pub fn new(position: Vec3, color: Color) -> Self {
        Self {
            position: position.to_array(),
            _padding: 0,
            color: [color.r(), color.g(), color.b()],
            _padding2: 0,
        }
    }
}

pub struct CameraBuffer(pub wgpu::Buffer);

pub struct LightBuffer(pub wgpu::Buffer);

pub struct MeshViewBindGroup(pub wgpu::BindGroup);

pub struct MeshViewBindGroupLayout(pub wgpu::BindGroupLayout);

pub fn setup_mesh_view_bind_group(
    mut commands: Commands,
    renderer: Res<WgpuRenderer>,
    camera_uniform: Res<CameraUniform>,
    light: Query<&LightUniform>,
) {
    let mesh_view_layout =
        renderer
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mesh_view_bind_group_layout"),
                entries: &[
                    // Camera
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Light
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

    let camera_buffer = renderer
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[*camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    let light_buffer = renderer
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[*light.single()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    let bind_group = renderer
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &mesh_view_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

    commands.insert_resource(CameraBuffer(camera_buffer));
    commands.insert_resource(LightBuffer(light_buffer));
    commands.insert_resource(MeshViewBindGroupLayout(mesh_view_layout));
    commands.insert_resource(MeshViewBindGroup(bind_group));
}

pub fn update_camera_buffer(
    renderer: Res<WgpuRenderer>,
    camera: Res<Camera>,
    camera_buffer: Res<CameraBuffer>,
    mut camera_uniform: ResMut<CameraUniform>,
) {
    if camera.is_changed() {
        camera_uniform.update_view_proj(&camera);
        renderer.queue.write_buffer(
            &camera_buffer.0,
            0,
            bytemuck::cast_slice(&[*camera_uniform]),
        );
    }
}

pub fn update_light_buffer(
    renderer: Res<WgpuRenderer>,
    mut query: Query<&mut LightUniform>,
    light_buffer: Res<LightBuffer>,
    time: Res<Time>,
) {
    for mut light in query.iter_mut() {
        let old_position = light.position;
        light.position =
            Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2 * time.delta_seconds())
                .mul_vec3(old_position.into())
                .to_array();

        renderer
            .queue
            .write_buffer(&light_buffer.0, 0, bytemuck::cast_slice(&[*light]));
    }
}
