struct Vertex {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] color: vec3<f32>;
    [[location(2)]] uv: vec2<f32>;
};


struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] color: vec3<f32>;
    [[location(1)]] uv: vec2<f32>;
};

[[stage(vertex)]]
fn vertex(
    model: Vertex,
) -> VertexOutput {
    var out: VertexOutput;
    out.uv = model.uv;
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}

[[group(0), binding(0)]]
var t_shadow: texture_depth_2d;
[[group(0), binding(1)]]
var s_shadow: sampler_comparison;

[[stage(fragment)]]
fn fragment(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let near = 0.1;
    let far = 100.0;
    let depth = textureSampleCompare(t_shadow, s_shadow, in.uv, in.clip_position.w);
    let linear_depth = (2.0 * near) / (far + near - depth * (far - near));
    return vec4<f32>(vec3<f32>(linear_depth), 1.0);
}