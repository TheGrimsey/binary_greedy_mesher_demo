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
#import bevy_pbr::mesh_view_types
#import bevy_pbr::mesh_bindings::mesh
#import bevy_pbr::pbr_types::{pbr_input_new, STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT};
#import bevy_pbr::prepass_utils


@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<storage, read> model_buffer: array<ModelQuad>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<storage, read> face_buffer: array<Face>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var textures: binding_array<texture_2d<f32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var nearest_sampler: sampler;

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

    // 3 bits for which face the quad is on (0-5).
    // Nearest corner (of the 4 face-corners) to each vertex, used for AO.
    // 2 bits per vertex, 4 vertices.
    // 11 bits total, packed into a u32.
    ao: u32
}

var<private> ambient_lerps: vec4<f32> = vec4<f32>(1.0,0.7,0.5,0.15);

fn x_positive_bits(bits: u32) -> u32{
    return (1u << bits) - 1u;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let first_vertex = mesh[vertex.instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;

    let face_id = vertex_index >> 2;
    let vertex_id = vertex_index & 3u;

    let face = face_buffer[face_id];
    let model_quad = model_buffer[face.model_id];

    let vertex_position = model_quad.positions[vertex_id];
    let vertex_uv = model_quad.uv[vertex_id];
    let normal = model_quad.normal;

    let face_x = f32(face.pos_ao & x_positive_bits(5u));
    let face_y = f32(face.pos_ao >> 5u & x_positive_bits(5u));
    let face_z = f32(face.pos_ao >> 10u & x_positive_bits(5u));

    // AO only for all corners of the voxel.
    // Need to use this to get the correct AO value for the face.
    let corner_index = (model_quad.ao >> (3u + vertex_id * 2u)) & x_positive_bits(2u);
    let ao = (face.pos_ao >> (15u + corner_index * 2u)) & x_positive_bits(2u);

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
    out.texture_id = face.texture_id;

    out.uv = vertex_uv;

    out.instance_index = vertex.instance_index;
    return out;
}

@fragment
fn fragment(input: VertexOutput) -> FragmentOutput {
    var pbr_input = pbr_input_new();

    pbr_input.flags = mesh[input.instance_index].flags | STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT;

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

    let color = textureSample(textures[input.texture_id], nearest_sampler, input.uv);
    pbr_input.material.base_color = vec4(color.xyz * input.ambient, color.w);
    //pbr_input.material.emissive = input.blend_emissive;

    pbr_input.material.reflectance = vec3(0.5, 0.5, 0.5);
    pbr_input.material.perceptual_roughness = 1.0;
    pbr_input.material.metallic = 0.01;


#ifdef PREPASS_PIPELINE
    // in deferred mode we can't modify anything after that, as lighting is run in a separate fullscreen shader.
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    // apply lighting
    out.color = apply_pbr_lighting(pbr_input);
    out.color = apply_fog(mesh_view_bindings::fog, out.color, pbr_input.world_position.xyz, mesh_view_bindings::view.world_position.xyz);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}

fn apply_fog(fog_params: mesh_view_types::Fog, input_color: vec4<f32>, fragment_world_position: vec3<f32>, view_world_position: vec3<f32>) -> vec4<f32> {
    let view_to_world = fragment_world_position.xyz - view_world_position.xyz;

    // `length()` is used here instead of just `view_to_world.z` since that produces more
    // high quality results, especially for denser/smaller fogs. we get a "curved"
    // fog shape that remains consistent with camera rotation, instead of a "linear"
    // fog shape that looks a bit fake
    let distance = length(view_to_world);

    var scattering = vec3<f32>(0.0);
    if fog_params.directional_light_color.a > 0.0 {
        let view_to_world_normalized = view_to_world / distance;
        let n_directional_lights = mesh_view_bindings::lights.n_directional_lights;
        for (var i: u32 = 0u; i < n_directional_lights; i = i + 1u) {
            let light = mesh_view_bindings::lights.directional_lights[i];
            scattering += pow(
                max(
                    dot(view_to_world_normalized, light.direction_to_light),
                    0.0
                ),
                fog_params.directional_light_exponent
            ) * light.color.rgb * mesh_view_bindings::view.exposure;
        }
    }

    if fog_params.mode == mesh_view_types::FOG_MODE_LINEAR {
        return bevy_pbr::fog::linear_fog(fog_params, input_color, distance, scattering);
    } else if fog_params.mode == mesh_view_types::FOG_MODE_EXPONENTIAL {
        return bevy_pbr::fog::exponential_fog(fog_params, input_color, distance, scattering);
    } else if fog_params.mode == mesh_view_types::FOG_MODE_EXPONENTIAL_SQUARED {
        return bevy_pbr::fog::exponential_squared_fog(fog_params, input_color, distance, scattering);
    } else if fog_params.mode == mesh_view_types::FOG_MODE_ATMOSPHERIC {
        return bevy_pbr::fog::atmospheric_fog(fog_params, input_color, distance, scattering);
    } else {
        return input_color;
    }
}
