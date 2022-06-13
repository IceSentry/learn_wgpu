[[group(0), binding(0)]]
var t_diffuse: texture_2d<f32>;
[[group(0), binding(1)]]
var s_diffuse: sampler;

struct CameraUniform {
    view_pos: vec4<f32>;
    view_proj: mat4x4<f32>;
};
[[group(1), binding(0)]]
var<uniform> camera: CameraUniform;

struct Light {
    position: vec3<f32>;
    color: vec3<f32>;
};
[[group(2), binding(0)]]
var<uniform> light: Light;

struct Vertex {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] uv: vec2<f32>;
    [[location(2)]] normal: vec3<f32>;
};

struct InstanceInput {
    [[location(5)]] transform_matrix_0: vec4<f32>;
    [[location(6)]] transform_matrix_1: vec4<f32>;
    [[location(7)]] transform_matrix_2: vec4<f32>;
    [[location(8)]] transform_matrix_3: vec4<f32>;
    [[location(9)]] normal_matrix_0: vec3<f32>;
    [[location(10)]] normal_matrix_1: vec3<f32>;
    [[location(11)]] normal_matrix_2: vec3<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] uv: vec2<f32>;
    [[location(1)]] world_normal: vec3<f32>;
    [[location(2)]] world_position: vec3<f32>;
};

[[stage(vertex)]]
fn vertex(
    vertex: Vertex,
    instance: InstanceInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.transform_matrix_0,
        instance.transform_matrix_1,
        instance.transform_matrix_2,
        instance.transform_matrix_3,
    );

    let normal_matrix = mat3x3<f32>(
        instance.normal_matrix_0,
        instance.normal_matrix_1,
        instance.normal_matrix_2,
    );

    var out: VertexOutput;
    out.uv = vertex.uv;
    out.world_normal = normal_matrix * vertex.normal;

    var world_position: vec4<f32> = model_matrix * vec4<f32>(vertex.position, 1.0);
    out.world_position = world_position.xyz;
    out.clip_position = camera.view_proj * world_position;
    return out;
}

[[stage(fragment)]]
fn fragment(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.uv);

    // TODO load ambient values from uniform buffer
    let ambient_strength = 0.05;
    let ambient_color = light.color * ambient_strength;

    let light_dir = normalize(light.position - in.world_position);

    let diffuse_strength = max(dot(in.world_normal, light_dir), 0.0);
    let diffuse_color = light.color * diffuse_strength;

    let view_dir = normalize(camera.view_pos.xyz - in.world_position);
    let half_dir = normalize(view_dir + light_dir);

    let specular_strength = pow(max(dot(in.world_normal, half_dir), 0.0), 32.0);
    let specular_color = specular_strength * light.color;

    let result = (ambient_color + diffuse_color + specular_color) * color.rgb;
    // let result = specular_color;
    // let result = specular_color * in.world_normal;
    // let result = in.world_normal;

    return vec4<f32>(result, color.a);
}
