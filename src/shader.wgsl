struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Uniforms {
    rect: vec4<f32>,
}

@group(0) @binding(0)
var tex_y: texture_2d<f32>;

@group(0) @binding(1)
var tex_uv: texture_2d<f32>;

@group(0) @binding(2)
var s: sampler;

@group(0) @binding(3)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var quad = array<vec4<f32>, 6>(
        vec4<f32>(uniforms.rect.xy, 0.0, 0.0),
        vec4<f32>(uniforms.rect.zy, 1.0, 0.0),
        vec4<f32>(uniforms.rect.xw, 0.0, 1.0),
        vec4<f32>(uniforms.rect.zy, 1.0, 0.0),
        vec4<f32>(uniforms.rect.zw, 1.0, 1.0),
        vec4<f32>(uniforms.rect.xw, 0.0, 1.0),
    );

    var out: VertexOutput;
    out.uv = quad[in_vertex_index].zw;
    out.position = vec4<f32>(quad[in_vertex_index].xy, 1.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex_y, s, in.uv);
}
