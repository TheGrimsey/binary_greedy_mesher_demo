#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip, mesh_normal_local_to_world}
#import bevy_pbr::pbr_functions::{calculate_view, prepare_world_normal}
#import bevy_pbr::mesh_view_bindings
#import bevy_pbr::mesh_bindings
#import bevy_pbr::mesh_bindings::mesh
#import bevy_pbr::pbr_types::pbr_input_new
#import bevy_pbr::prepass_utils

struct ChunkMaterial {
    reflectance: f32,
    perceptual_roughness: f32,
    metallic: f32,
};

@group(2) @binding(0) var<uniform> chunk_material: ChunkMaterial;
@group(2) @binding(1) var<storage, read> model_buffer: array<ModelQuad>;
@group(2) @binding(2) var<storage, read> face_buffer: array<Face>;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @builtin(vertex_index) index: u32
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) ambient: f32,
    @location(4) instance_index: u32,
    @location(5) texture_id: u32,
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

var<private> ambient_lerps: vec4<f32> = vec4<f32>(1.0,0.7,0.5,0.15);

fn x_positive_bits(bits: u32) -> u32{
    return (1u << bits) - 1u;
}

/*
*   Vertex Buffer:
    - X,Y,Z - 6 bits each
    - AO - 3 bits

*   Face Buffer:
*   -  
*   Model Buffer?
*   - Face Normals
*   - Face Colors
*/

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let face_id = vertex.index >> 2;
    let vertex_id = vertex.index & 3u;

    let face = face_buffer[face_id];
    let model = model_buffer[face.model_id];

    let vertex_position = model.positions[vertex_id];
    let vertex_uv = model.uv[vertex_id];
    let normal = model.normal;

    let face_x = f32(face.pos_ao & x_positive_bits(5u));
    let face_y = f32(face.pos_ao >> 5u & x_positive_bits(5u));
    let face_z = f32(face.pos_ao >> 10u & x_positive_bits(5u));

    // AO only for all corners of the voxel.
    // Need to use this to get the correct AO value for the face.
    let model_ao_index = (model.ao >> (vertex_id * 3u)) & 3u;

    let ao = face.pos_ao >> (15u + model_ao_index * 2u) & 2u;

    let x = face_x + vertex_position.x;
    let y = face_y + vertex_position.y;
    let z = face_z + vertex_position.z;

    let local_position = vec4<f32>(x,y,z, 1.0);
    let world_position = get_world_from_local(vertex.instance_index) * local_position;
    out.clip_position = mesh_position_local_to_clip(
        get_world_from_local(vertex.instance_index),
        local_position,
    );

    let ambient_lerp = ambient_lerps[ao];
    out.ambient = ambient_lerp;
    out.world_position = world_position;
    
    out.world_normal = mesh_normal_local_to_world(normal, vertex.instance_index);

    out.uv = vertex_uv;

    out.instance_index = vertex.instance_index;
    return out;
}

@fragment
fn fragment(input: VertexOutput) -> FragmentOutput {
    var pbr_input = pbr_input_new();

    pbr_input.flags = mesh[input.instance_index].flags;

    pbr_input.V = calculate_view(input.world_position, false);
    pbr_input.frag_coord = input.clip_position;
    pbr_input.world_position = input.world_position;

    pbr_input.world_normal = prepare_world_normal(
        input.world_normal,
        false,
        false,
    );
#ifdef LOAD_PREPASS_NORMALS
    pbr_input.N = prepass_utils::prepass_normal(input.clip_position, 0u);
#else
    pbr_input.N = normalize(pbr_input.world_normal);
#endif

    pbr_input.material.base_color = vec4<f32>(input.blend_color.xyz * input.ambient, input.blend_color.w);
    pbr_input.material.emissive = input.blend_emissive;

    pbr_input.material.reflectance = chunk_material.reflectance;
    pbr_input.material.perceptual_roughness = chunk_material.perceptual_roughness;
    pbr_input.material.metallic = chunk_material.metallic;


#ifdef PREPASS_PIPELINE
    // in deferred mode we can't modify anything after that, as lighting is run in a separate fullscreen shader.
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    // apply lighting
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
