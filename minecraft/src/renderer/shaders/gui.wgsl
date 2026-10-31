struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

struct VertexInput {
    // instance data, per quad/6 verticies
    @location(0) pos: vec2f,
    @location(1) uv: vec2f,
}

struct Camera {
    view_proj: mat4x4<f32>,
    screen_size: vec2f,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(i: VertexInput) -> VertexOutput {
    // convert to screen coordinates

    let aspect_ratio = camera.screen_size.x / camera.screen_size.y;
    var pos = i.pos;
    if aspect_ratio > 1.0 {
        pos.x /= aspect_ratio;
    } else {
        pos.y *= aspect_ratio;
    }

    return VertexOutput(vec4f(pos.xy, 0.0, 1.0), i.uv);
}

@group(0) @binding(3)
var atlas: texture_2d<f32>;

@group(0) @binding(4)
var atlas_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let size = vec2<f32>(textureDimensions(atlas));

    // return vec4f(1.0, 0.0, 0.0, 1.0);
    return textureSample(atlas, atlas_sampler, in.uv / size);
}
