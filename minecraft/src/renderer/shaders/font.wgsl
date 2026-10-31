struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec4f,
}

struct VertexInput {
    // instance data, per quad/6 verticies
    @location(0) pos: vec2f,
    @location(1) uv: vec2<u32>,
    @location(2) color: vec4f,
}

struct Camera {
    view_proj: mat4x4<f32>,
    screen_size: vec2f,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(i: VertexInput) -> VertexOutput {
    let pos = i.pos / camera.screen_size;

    return VertexOutput(vec4f(pos.xy, 0.0, 1.0), vec2f(i.uv), i.color);
}

@group(0) @binding(3)
var atlas: texture_2d<f32>;

@group(0) @binding(4)
var atlas_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let size = vec2<f32>(textureDimensions(atlas));

    return textureSample(atlas, atlas_sampler, in.uv / size) * in.color;
}
