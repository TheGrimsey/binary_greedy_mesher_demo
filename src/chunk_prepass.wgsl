#import bevy_pbr::{
    prepass_bindings,
    mesh_functions,
    prepass_io::{FragmentOutput},
    skinning,
    morph,
    mesh_view_bindings::{view, previous_view_proj},
    view_transformations::position_world_to_clip,
}

#ifdef DEFERRED_PREPASS
#import bevy_pbr::rgb9e5
#endif

#import bevy_pbr::mesh_functions::{mesh_normal_local_to_world}
#import bevy_render::instance_index::{get_instance_index}


struct ChunkMaterial {
    reflectance: f32,
    perceptual_roughness: f32,
    metallic: f32,
    // _padding: f32,
};

struct Face {
    /// Block Position: X,Y,Z - 5 bits each (0-31)
    /// AO - 2 bits * 8 (one for each corner of the voxel)
    /// Final 1 bit unused.
    pos_ao: u32,

    /// Index into the model buffer
    model_id: u32,
    texture_id: u32,
}

struct ModelQuad {
    positions: array<vec3<f32>, 4>,
    uv: array<vec2<f32>, 4>,
    normal: vec3<f32>,

    // AO corner (of the 8 corners) for each vertex.
    // 3 bits per vertex, 4 vertices.
    // 12 bits total, packed into a u32.
    ao: u32
}

@group(2) @binding(0) var<uniform> material: ChunkMaterial;
@group(2) @binding(1) var<storage, read> model_buffer: array<ModelQuad>;
@group(2) @binding(2) var<storage, read> face_buffer: array<Face>;

fn x_positive_bits(bits: u32) -> u32{
    return (1u << bits) - 1u;
}

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @builtin(vertex_index) index: u32
};

struct MyVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec4<f32>,
    @location(2) clip_position_unclamped: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> MyVertexOutput {
    var out: MyVertexOutput;

    
    let face_id = vertex.index >> 2;
    let vertex_id = vertex.index & 3u;

    let face = face_buffer[face_id];
    let model_quad = model_buffer[face.model_id];

    let vertex_position = model_quad.positions[vertex_id];
    let vertex_uv = model_quad.uv[vertex_id];
    let normal = model_quad.normal;

    let face_x = f32(face.pos_ao & x_positive_bits(5u));
    let face_y = f32(face.pos_ao >> 5u & x_positive_bits(5u));
    let face_z = f32(face.pos_ao >> 10u & x_positive_bits(5u));

    let x = face_x + vertex_position.x;
    let y = face_y + vertex_position.y;
    let z = face_z + vertex_position.z;

    out.world_normal = mesh_normal_local_to_world(normal, vertex.instance_index);

    let local_position = vec4<f32>(x,y,z, 1.0);

    // Use vertex_no_morph.instance_index instead of vertex.instance_index to work around a wgpu dx12 bug.
    // See https://github.com/gfx-rs/naga/issues/2416
    var model = mesh_functions::get_world_from_local(vertex.instance_index);

    let world_position = mesh_functions::mesh_position_local_to_world(model, local_position);
    out.position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position;

#ifdef DEPTH_CLAMP_ORTHO
    out.clip_position_unclamped = out.position;
    out.position.z = min(out.position.z, 1.0);
#endif // DEPTH_CLAMP_ORTHO

    return out;
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: MyVertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

#ifdef DEPTH_CLAMP_ORTHO
    out.frag_depth = in.clip_position_unclamped.z;
#endif // DEPTH_CLAMP_ORTHO

#ifdef NORMAL_PREPASS
    out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
#endif

#ifdef DEFERRED_PREPASS
    // There isn't any material info available for this default prepass shader so we are just writing 
    // emissive magenta out to the deferred gbuffer to be rendered by the first deferred lighting pass layer.
    // This is here so if the default prepass fragment is used for deferred magenta will be rendered, and also
    // as an example to show that a user could write to the deferred gbuffer if they were to start from this shader.
    out.deferred = vec4(0u, bevy_pbr::rgb9e5::vec3_to_rgb9e5_(vec3(1.0, 0.0, 1.0)), 0u, 0u);
    out.deferred_lighting_pass_id = 1u;
#endif

    return out;
}
#endif // PREPASS_FRAGMENT
