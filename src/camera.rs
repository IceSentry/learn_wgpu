use bevy::{
    input::mouse::MouseMotion,
    math::{const_vec3, vec3},
    prelude::*,
};

use crate::{bind_groups::mesh_view::update_camera_buffer, renderer::WgpuRenderer};

const CAMERRA_EYE: Vec3 = const_vec3!([0.0, 5.0, 8.0]);
const MAX_SPEED: f32 = 15.0;
const FRICTION: f32 = 0.5;

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup_camera)
            .add_system(fly_camera)
            .add_system(update_camera_buffer);
    }
}

pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub aspect: f32,
    pub fov_y: f32,
    pub z_near: f32,
    pub z_far: f32,
    pub rotation: Quat,
}

impl Camera {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            eye: CAMERRA_EYE,
            target: vec3(0.0, 0.0, 0.0),
            up: Vec3::Y,
            aspect: width / height,
            fov_y: 45.0,
            z_near: 0.1,
            z_far: 1000.0,
            rotation: Quat::default(),
        }
    }

    pub fn build_view_projection_matrix(&self) -> Mat4 {
        let view = Mat4::from_rotation_translation(self.rotation, self.eye);
        let proj = Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far);
        proj * view.inverse()
    }

    #[inline]
    pub fn forward(&self) -> Vec3 {
        -self.local_z()
    }

    #[inline]
    pub fn right(&self) -> Vec3 {
        self.local_x()
    }

    #[inline]
    pub fn local_x(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    #[allow(unused)]
    #[inline]
    pub fn local_y(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    #[inline]
    pub fn local_z(&self) -> Vec3 {
        self.rotation * Vec3::Z
    }
}

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

fn setup_camera(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let camera = Camera::new(renderer.config.width as f32, renderer.config.height as f32);

    let mut camera_uniform = CameraUniform::new();
    camera_uniform.update_view_proj(&camera);

    commands.insert_resource(camera);
    commands.insert_resource(camera_uniform);
}

#[allow(clippy::too_many_arguments)]
fn fly_camera(
    time: Res<Time>,
    windows: Res<Windows>,
    mouse_input: Res<Input<MouseButton>>,
    key_input: Res<Input<KeyCode>>,
    mut camera: ResMut<Camera>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut velocity: Local<Vec3>,
) {
    if !mouse_input.pressed(MouseButton::Right) {
        return;
    }

    let dt = time.delta_seconds();

    // Rotate

    let mut mouse_delta = Vec2::ZERO;
    for mouse_motion in mouse_motion.iter() {
        mouse_delta += mouse_motion.delta;
    }

    if mouse_delta != Vec2::ZERO {
        let window = if let Some(window) = windows.get_primary() {
            Vec2::new(window.width() as f32, window.height() as f32)
        } else {
            Vec2::ZERO
        };
        let delta_x = mouse_delta.x / window.x * std::f32::consts::TAU;
        let delta_y = mouse_delta.y / window.y * std::f32::consts::PI;
        let yaw = Quat::from_rotation_y(-delta_x);
        let pitch = Quat::from_rotation_x(-delta_y);
        camera.rotation = yaw * camera.rotation; // rotate around global y axis
        camera.rotation *= pitch; // rotate around local x axis
    }

    // Translate

    let mut axis_input = Vec3::ZERO;
    if key_input.pressed(KeyCode::W) {
        axis_input.z += 1.0;
    }
    if key_input.pressed(KeyCode::S) {
        axis_input.z -= 1.0;
    }
    if key_input.pressed(KeyCode::D) {
        axis_input.x += 1.0;
    }
    if key_input.pressed(KeyCode::A) {
        axis_input.x -= 1.0;
    }
    if key_input.pressed(KeyCode::Space) {
        axis_input.y += 1.0;
    }
    if key_input.pressed(KeyCode::LShift) {
        axis_input.y -= 1.0;
    }

    if axis_input != Vec3::ZERO {
        *velocity = axis_input.normalize() * MAX_SPEED;
    } else {
        *velocity *= 1.0 - FRICTION;
        if velocity.length_squared() < 1e-6 {
            *velocity = Vec3::ZERO;
        }
    }

    let forward = camera.forward();
    let right = camera.right();
    camera.eye += velocity.x * dt * right + velocity.y * dt * Vec3::Y + velocity.z * dt * forward;
}
