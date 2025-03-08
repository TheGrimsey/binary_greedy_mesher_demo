use std::{f32::consts::PI, sync::Arc};

use bevy::{
    color::palettes::css, core::TaskPoolThreadAssignmentPolicy, core_pipeline::oit::OrderIndependentTransparencySettings, math::ivec3, pbr::CascadeShadowConfigBuilder, prelude::*, render::{
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
        RenderPlugin,
    }
};

use bevy_inspector_egui::quick::{AssetInspectorPlugin, WorldInspectorPlugin, ResourceInspectorPlugin};
use bevy_screen_diagnostics::{
    ScreenDiagnosticsPlugin, ScreenEntityDiagnosticsPlugin, ScreenFrameDiagnosticsPlugin,
};

use new_voxel_testing::{
    chunk::{self, ChunkGenerator}, diagnostics::VoxelDiagnosticsPlugin, rendering::{
        ChunkMaterial,
        RenderingPlugin,
    }, scanner::{DataScanner, MeshScanner, Scanner}, sun::{Sun, SunPlugin, SunSettings}, utils::world_to_chunk, voxel::*, voxel_engine::{ChunkModification, VoxelEngine, VoxelEnginePlugin}
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
        .add_plugins(ResourceInspectorPlugin::<SunSettings>::default())
        .add_plugins(VoxelEnginePlugin)
        .add_plugins(SunPlugin)
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

    let _ = block_registry.add_block(BlockStringIdentifier(Box::from("stone")), &Block { visibility: BlockVisibilty::Solid, color: Color::srgba(0.1, 0.1, 0.1, 1.0), ..default() });

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
        Sun,
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::ZYX,
            0.0,
            PI / 2.,
            -PI / 4.,
        )),
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
            OrderIndependentTransparencySettings::default()
        ))
        .insert(FlyCam);

    // circular base in origin
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(22.0))),
        MeshMaterial3d(materials.add(Color::from(css::GREEN))),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    commands.insert_resource(ChunkGenerator {
        generate: Arc::new(chunk::generate)
    });
}
