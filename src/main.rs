use std::f32::consts::PI;

use bevy::{
    color::palettes::css, core::TaskPoolThreadAssignmentPolicy, core_pipeline::oit::OrderIndependentTransparencySettings, math::ivec3, pbr::CascadeShadowConfigBuilder, prelude::*, render::{
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
        RenderPlugin,
    }
};

use bevy_inspector_egui::quick::{AssetInspectorPlugin, WorldInspectorPlugin};
use bevy_screen_diagnostics::{
    ScreenDiagnosticsPlugin, ScreenEntityDiagnosticsPlugin, ScreenFrameDiagnosticsPlugin,
};

use new_voxel_testing::{
    rendering::{
        ChunkMaterial, ChunkMaterialWireframe, GlobalChunkWireframeMaterial,
        RenderingPlugin,
    }, scanner::{DataScanner, MeshScanner, ScannerTwo}, sun::{Sun, SunPlugin}, utils::world_to_chunk, voxel::*, voxel_engine::{ChunkModification, VoxelEngine, VoxelEnginePlugin}
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
        .add_plugins(SunPlugin)
        .add_systems(Startup, setup)
        // camera plugin
        .add_plugins(NoCameraPlayerPlugin)
        .add_plugins(RenderingPlugin)
        .add_plugins((
            ScreenDiagnosticsPlugin::default(),
            ScreenFrameDiagnosticsPlugin,
            ScreenEntityDiagnosticsPlugin,
        ))
        .insert_resource(MovementSettings {
            sensitivity: 0.00015, // default: 0.00012
            speed: 64.0 * 2.0,    // default: 12.0
                                  // speed: 32.0 * 12.0,   // default: 12.0
        })
        .add_systems(Update, modify_current_terrain)
        .run();
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
    mut chunk_materials_wireframe: ResMut<Assets<ChunkMaterialWireframe>>,
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
        ScannerTwo::<DataScanner>::new(10),
        ScannerTwo::<MeshScanner>::new(9), 
    ));

    commands
        .spawn((
            ScannerTwo::<DataScanner>::new(33),
            ScannerTwo::<MeshScanner>::new(32), 
            Camera3d::default(),
            Transform::from_xyz(0.0, 2.0, 0.5),
            Msaa::Off,
            OrderIndependentTransparencySettings::default()
        ))
        .insert(FlyCam);

    commands.insert_resource(GlobalChunkWireframeMaterial(chunk_materials_wireframe.add(
        ChunkMaterialWireframe {
            reflectance: 0.5,
            perceptual_roughness: 1.0,
            metallic: 0.01,
        },
    )));

    // circular base in origin
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(22.0))),
        MeshMaterial3d(materials.add(Color::from(css::GREEN))),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
}
