use std::{f32::consts::PI, sync::Arc};

use bevy::{
    color::palettes::css, core::TaskPoolThreadAssignmentPolicy, core_pipeline::oit::OrderIndependentTransparencySettings, math::ivec3, pbr::CascadeShadowConfigBuilder, prelude::*, render::{
        settings::{RenderCreation, WgpuFeatures, WgpuSettings}, view::NoFrustumCulling, RenderPlugin
    }
};

use bevy_inspector_egui::quick::{AssetInspectorPlugin, WorldInspectorPlugin, ResourceInspectorPlugin};
use bevy_screen_diagnostics::{
    ScreenDiagnosticsPlugin, ScreenEntityDiagnosticsPlugin, ScreenFrameDiagnosticsPlugin,
};

use bracket_noise::prelude::FastNoise;
use new_voxel_testing::{
    chunk::{self, ChunkData, ChunkGenerator, NoiseDownSampler2D, NoiseDownSampler3D}, constants::CHUNK_SIZE3, diagnostics::VoxelDiagnosticsPlugin, rendering::{
        ChunkMaterial,
        RenderingPlugin,
    }, scanner::{DataScanner, MeshScanner, Scanner}, utils::{index_to_ivec3, world_to_chunk}, voxel::*, voxel_engine::{ChunkModification, VoxelEngine, VoxelEnginePlugin}
};

use bevy_flycam::prelude::*;
use rand::Rng;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    // WARN this is a native only feature. It will not work with webgl or webgpu
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    ..default()
                }),
                ..default()
            })
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions {
                    async_compute: TaskPoolThreadAssignmentPolicy {
                        min_threads: 1,
                        max_threads: 8,
                        percent: 0.75,
                    },
                    ..default()
                },
            }),))
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(AssetInspectorPlugin::<ChunkMaterial>::default())
        .add_plugins(VoxelEnginePlugin)
        .add_systems(Startup, setup)
        // camera plugin
        .add_plugins(NoCameraPlayerPlugin)
        .add_plugins(RenderingPlugin)
        .add_plugins((
            ScreenDiagnosticsPlugin::default(),
            VoxelDiagnosticsPlugin,
            ScreenFrameDiagnosticsPlugin,
            ScreenEntityDiagnosticsPlugin,
        ))
        .insert_resource(MovementSettings {
            sensitivity: 0.00015, // default: 0.00012
            speed: 64.0 * 2.0,    // default: 12.0
                                  // speed: 32.0 * 12.0,   // default: 12.0
        })
        .add_systems(Update, modify_current_terrain)
        .add_systems(PreStartup, load_block_registry)
        .run();
}

fn load_block_registry(
    mut commands: Commands,
) {
    // TODO: Actually load a block registry from assets. For now, just add some dummy blocks.
    let mut block_registry = BlockRegistry::default();
    let _ = block_registry.add_block(
        BlockStringIdentifier(Box::from("air")),
        &Block { visibility: BlockVisibilty::Invisible, collision: false, ..default() },
    );
    let _ = block_registry.add_block(BlockStringIdentifier(Box::from("dirt")), &Block { visibility: BlockVisibilty::Solid, color: Color::srgb(0.0, 1.0, 0.0), ..default() });
    let _ = block_registry.add_block(BlockStringIdentifier(Box::from("grass")), &Block { visibility: BlockVisibilty::Solid, color: Color::srgb(0.3, 0.4, 0.0), ..default() });

    let _ = block_registry.add_block(BlockStringIdentifier(Box::from("glass")), &Block { visibility: BlockVisibilty::Transparent, color: Color::srgba(0.3, 0.3, 0.3, 0.5), ..default() });

    let _ = block_registry.add_block(BlockStringIdentifier(Box::from("stone")), &Block { visibility: BlockVisibilty::Solid, color: Color::srgba(1.0, 1.0, 1.0, 1.0), ..default() });

    commands.insert_resource(BlockRegistryResource(Arc::new(block_registry)));
}

pub fn modify_current_terrain(
    query: Query<&Transform, With<Camera>>,
    key: Res<ButtonInput<KeyCode>>,
    mut voxel_engine: ResMut<VoxelEngine>,
) {
    if !key.pressed(KeyCode::KeyN) {
        return;
    }
    let cam_transform = query.single();
    let cam_chunk = world_to_chunk(cam_transform.translation + (cam_transform.forward() * 64.0));

    let mut rng = rand::rng();
    let mut mods = vec![];
    for _i in 0..32 * 32 {
        let pos = ivec3(
            rng.random_range(0..32),
            rng.random_range(0..32),
            rng.random_range(0..32),
        );
        mods.push(ChunkModification(pos, BlockId(0)));
    }
    voxel_engine.chunk_modifications.insert(cam_chunk, mods);
}

pub fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn((
        Name::new("directional light light"),
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_translation(Vec3::new(-10.0, 10.0, -10.0)).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 32.0 * 20.0,
            ..default()
        }.build()
    ));
    // uncomment for scanner at origin position
    commands.spawn((
        Transform::default(),
        Scanner::<DataScanner>::new(10, Some(5)),
        Scanner::<MeshScanner>::new(9, Some(4)), 
    ));

    commands
        .spawn((
            Scanner::<DataScanner>::new(16, Some(7)),
            Scanner::<MeshScanner>::new(15, Some(6)), 
            Camera3d::default(),
            Transform::from_xyz(0.0, 2.0, 0.5),
            Msaa::Off,
            OrderIndependentTransparencySettings::default(),
            FlyCam
        ));

    // circular base in origin
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(22.0))),
        MeshMaterial3d(materials.add(Color::from(css::GREEN))),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    commands.insert_resource(ChunkGenerator {
        generate: Arc::new(generate)
    });
}


/// shape our voxel data based on the chunk_pos
pub fn generate(chunk_pos: IVec3) -> ChunkData {

    // hardcoded extremity check
    let chunk_height_limit = 3;

    if chunk_pos.y > chunk_height_limit {
        return ChunkData {
            voxels: vec![BlockData {
                block_type: BlockId(0),
            }],
        };
    }
    // hardcoded extremity check
    if chunk_pos.y < -chunk_height_limit {
        return ChunkData {
            voxels: vec![BlockData {
                block_type: BlockId(2),
            }],
        };
    }

    let _span = info_span!("Generating chunk data").entered();

    let chunk_origin = chunk_pos * 32;
    let mut voxels = Vec::with_capacity(CHUNK_SIZE3);

    let mut continental_noise = FastNoise::seeded(37);
    continental_noise.set_frequency(0.0002591);

    let continental_noise_downsampler = NoiseDownSampler2D::new(5, &continental_noise, chunk_origin.xz(), 55.0, None, false);

    let mut errosion = FastNoise::seeded(549);
    errosion.set_frequency(0.004891);

    let errosion_downsampler = NoiseDownSampler2D::new(5, &errosion, chunk_origin.xz(), 1.0, None, false);

    let mut fast_noise = FastNoise::new();
    fast_noise.set_frequency(0.002591);
    let surface_noise = NoiseDownSampler2D::new(1, &fast_noise, chunk_origin.xz(), 30.0, None, false);
    
    fast_noise.set_frequency(0.0254);
    let overhang_downsamper = NoiseDownSampler3D::new(1, &fast_noise, chunk_origin, 55.0, Some(IVec3::new(0, 12, 0)));

    for i in 0..CHUNK_SIZE3 {
        let voxel_pos = chunk_origin + index_to_ivec3(i);

        let overhang = overhang_downsamper.get_noise(voxel_pos);
        let noise_2 = surface_noise.get_noise(voxel_pos.xz());

        let errosion_noise = errosion_downsampler.get_noise(voxel_pos.xz());
        let continental_noise = continental_noise_downsampler.get_noise(voxel_pos.xz());

        let surface_height = continental_noise + (noise_2 + overhang) * (1.0 - errosion_noise);
        let solid = surface_height > voxel_pos.y as f32;

        let block_type = match solid {
            true => match surface_height - voxel_pos.y as f32 { // Distance from surface
                y if y > 3.0 => BlockId(4), // Stone
                y if y > 1.0 => BlockId(1), // Dirt
                _ => BlockId(2), // Grass
            },
            false => {
                BlockId(0)
            },
        };
        voxels.push(BlockData { block_type });
    }

    ChunkData { voxels }
}
