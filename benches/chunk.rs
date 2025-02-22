use bevy::prelude::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use new_voxel_testing::chunk::ChunkData;

fn bench_chunk(world_pos: IVec3) {
    let _chunk = ChunkData::generate(world_pos);
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("build chunk data", |b| {
        b.iter_with_setup(
            || {
                use rand::Rng;
                let mut rng = rand::rng();
                let b = 100;
                let y = 20;
                black_box(IVec3::new(
                    rng.random_range(-b..b),
                    rng.random_range(-y..y),
                    rng.random_range(-b..b),
                ))
            },
            bench_chunk,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
