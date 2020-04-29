#version 450

layout (location = 0) out vec4 vertex_color;

const vec2 positions[3] = vec2[3](
    vec2(0.0, 0.5),
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5)
);

void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    vertex_color = vec4(
        gl_VertexIndex == 0 ? 1.0 : 0.0, 
        gl_VertexIndex == 1 ? 1.0 : 0.0, 
        gl_VertexIndex == 2 ? 1.0 : 0.0, 
        1.0
    );
}