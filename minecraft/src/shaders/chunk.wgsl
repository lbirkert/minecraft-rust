struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) tint: vec4f,
    @location(2) world_pos: vec4f,
    @location(3) normal: vec3f,
    @interpolate(flat)
    @location(4) is_selected: u32,
    // TODO: proper lighting
}

struct VertexInput {
    @builtin(vertex_index) vi: u32,

    // instance data, per quad/6 verticies
    @location(0) block_id: u32,
    @location(1) quad_id: u32,
}

struct Camera {
    view_proj: mat4x4<f32>,
}

struct Chunk {
    bottom_left: vec4f, // bottom left chunk pos
    selected_block: u32,
}

struct Quad {
    v1: QuadVertex,
    v2: QuadVertex,
    v3: QuadVertex,
    v4: QuadVertex,
    tint: vec4f,
}

struct QuadVertex {
    delta: u32, // u8xu8xu8 (normalized to f32 by 1/0xFF)
    uv: u32, // u16xu16
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(0) @binding(1)
var<uniform> chunk: Chunk;

@group(0) @binding(2)
var<storage, read> quads: array<Quad>;

fn get_vertex(quad: Quad, vi: u32) -> QuadVertex {
    switch vi % 6 {
        case 0 { return quad.v1; }
        case 1 { return quad.v2; }
        case 2 { return quad.v3; }
        case 3 { return quad.v1; }
        case 4 { return quad.v3; }
        case 5 { return quad.v4; }
        default { return quad.v1; }
    }
}

fn unpack_uv(packed: u32) -> vec2<f32> {
    let u = packed & 0xFFFFu;
    let v = packed >> 16u;

    return vec2<f32>(f32(u), f32(v)) / 16.0;
}

fn unpack_delta(packed: u32) -> vec3f {
    let vdx = f32(packed & 0xFF) / 0xFF;
    let vdy = f32((packed >> 8) & 0xFF) / 0xFF;
    let vdz = f32((packed >> 16) & 0xFF) / 0xFF;
    return vec3f(vdx, vdy, vdz);
}

const CHUNK_WIDTH_X: u32 = 16u;
const CHUNK_WIDTH_Z: u32 = 16u;

fn unpack_block_id(block_id: u32) -> vec3f {
    let x = block_id % CHUNK_WIDTH_X;
    let z = (block_id / CHUNK_WIDTH_X) % CHUNK_WIDTH_Z;
    let y = block_id / (CHUNK_WIDTH_X * CHUNK_WIDTH_Z);

    return vec3f(f32(x), f32(y), f32(z));
}

@vertex
fn vs_main(i: VertexInput) -> VertexOutput {
    let block_pos = chunk.bottom_left + vec4f(unpack_block_id(i.block_id), 1.0);

    let quad = quads[i.quad_id];
    let vertex = get_vertex(quad, i.vi);

    let vertex_delta = unpack_delta(vertex.delta);
    let vertex_world_pos = block_pos + vec4f(vertex_delta, 0.0);

    // convert to screen coordinates
    let vertex_pos = camera.view_proj * vertex_world_pos;

    let vertex_uv = unpack_uv(vertex.uv);

    var lighting: f32;
    let d1 = unpack_delta(quad.v1.delta);
    let d2 = unpack_delta(quad.v2.delta);
    let d3 = unpack_delta(quad.v3.delta);

    let normal = normalize(cross(d3 - d2, d1 - d2));

    var is_selected = 0u;
    if chunk.selected_block == i.block_id {
        is_selected = 1;
    }

    return VertexOutput(vertex_pos, vertex_uv, quad.tint, vertex_world_pos, normal, is_selected);
}

@group(0) @binding(3)
var atlas: texture_2d<f32>;

@group(0) @binding(4)
var atlas_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let size = vec2<f32>(textureDimensions(atlas));

    var color: vec4f = textureSample(atlas, atlas_sampler, in.uv / size);
    color *= in.tint;
    color *= dot(in.normal, vec3f(0.0, 1.0, 0.0)) * 0.5 + 0.5;

    if in.is_selected == 1 {
        color += vec4f(0.5, 0.5, 0.5, 0.0);
    }

    return color;
}
