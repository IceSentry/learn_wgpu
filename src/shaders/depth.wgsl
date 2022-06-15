struct Vertex {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] uv: vec2<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(1)]] uv: vec2<f32>;
};

[[stage(vertex)]]
fn vertex(
    in: Vertex,
) -> VertexOutput {
    var out: VertexOutput;
    out.uv = in.uv;
    out.clip_position = vec4<f32>(in.position, 1.0);
    return out;
}

[[group(0), binding(0)]]
var depth_texture: texture_depth_2d;
[[group(0), binding(1)]]
var depth_sampler: sampler;

[[stage(fragment)]]
fn fragment(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let near = 0.1;
    let far = 1000.0;
    let depth = textureSample(depth_texture, depth_sampler, in.uv);
    let linear_depth = (2.0 * near) / (far + near - depth * (far - near));
    return vec4<f32>(vec3<f32>(linear_depth), 1.0);
}