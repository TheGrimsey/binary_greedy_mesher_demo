use bevy::{app::{App, Plugin, Startup, Update}, diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic}, ecs::system::{Res, ResMut}};
use bevy_screen_diagnostics::{Aggregate, ScreenDiagnostics};

use crate::{rendering::MeshingPipeline, voxel_engine::VoxelEngine};

const DIAG_LOAD_DATA_QUEUE: DiagnosticPath = DiagnosticPath::const_new("load_data_queue");
const DIAG_UNLOAD_DATA_QUEUE: DiagnosticPath = DiagnosticPath::const_new("unload_data_queue");
const DIAG_LOAD_MESH_QUEUE: DiagnosticPath = DiagnosticPath::const_new("load_mesh_queue");
const DIAG_UNLOAD_MESH_QUEUE: DiagnosticPath = DiagnosticPath::const_new("unload_mesh_queue");
const DIAG_VERTEX_COUNT: DiagnosticPath = DiagnosticPath::const_new("vertex_count");
const DIAG_MESH_TASKS: DiagnosticPath = DiagnosticPath::const_new("mesh_tasks");
const DIAG_DATA_TASKS: DiagnosticPath = DiagnosticPath::const_new("data_tasks");

pub struct VoxelDiagnosticsPlugin;
impl Plugin for VoxelDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_diagnostics);
        app.register_diagnostic(Diagnostic::new(DIAG_LOAD_MESH_QUEUE));
        app.register_diagnostic(Diagnostic::new(DIAG_UNLOAD_MESH_QUEUE));
        app.register_diagnostic(Diagnostic::new(DIAG_LOAD_DATA_QUEUE));
        app.register_diagnostic(Diagnostic::new(DIAG_UNLOAD_DATA_QUEUE));
        app.register_diagnostic(Diagnostic::new(DIAG_VERTEX_COUNT));
        app.register_diagnostic(Diagnostic::new(DIAG_MESH_TASKS));
        app.register_diagnostic(Diagnostic::new(DIAG_DATA_TASKS));
        app.add_systems(Update, diagnostics_count);
    }
}

fn setup_diagnostics(mut onscreen: ResMut<ScreenDiagnostics>) {
    onscreen
        .add("load_data_queue".to_string(), DIAG_LOAD_DATA_QUEUE)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>4.0}"));
    onscreen
        .add("unload_data_queue".to_string(), DIAG_UNLOAD_DATA_QUEUE)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>3.0}"));
    onscreen
        .add("load_mesh_queue".to_string(), DIAG_LOAD_MESH_QUEUE)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>4.0}"));
    onscreen
        .add("unload_mesh_queue".to_string(), DIAG_UNLOAD_MESH_QUEUE)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>3.0}"));
    onscreen
        .add("vertex_count".to_string(), DIAG_VERTEX_COUNT)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>7.0}"));
    onscreen
        .add("mesh_tasks".to_string(), DIAG_MESH_TASKS)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>4.0}"));
    onscreen
        .add("data_tasks".to_string(), DIAG_DATA_TASKS)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>2.0}"));
}

fn diagnostics_count(mut diagnostics: Diagnostics, voxel_engine: Res<VoxelEngine>, mesh_pipeline: Res<MeshingPipeline>) {
    diagnostics.add_measurement(&DIAG_LOAD_DATA_QUEUE, || {
        voxel_engine.load_data_queue.len() as f64
    });
    diagnostics.add_measurement(&DIAG_UNLOAD_DATA_QUEUE, || {
        voxel_engine.unload_data_queue.len() as f64
    });
    diagnostics.add_measurement(&DIAG_LOAD_MESH_QUEUE, || {
        mesh_pipeline.load_mesh_queue.len() as f64
    });
    diagnostics.add_measurement(&DIAG_UNLOAD_MESH_QUEUE, || {
        mesh_pipeline.unload_mesh_queue.len() as f64
    });
    diagnostics.add_measurement(&DIAG_MESH_TASKS, || mesh_pipeline.mesh_tasks.len() as f64);
    diagnostics.add_measurement(&DIAG_DATA_TASKS, || voxel_engine.data_tasks.len() as f64);
    diagnostics.add_measurement(&DIAG_VERTEX_COUNT, || {
        mesh_pipeline
            .vertex_diagnostic
            .iter()
            .map(|(_, v)| v)
            .sum::<i32>() as f64
    });
}