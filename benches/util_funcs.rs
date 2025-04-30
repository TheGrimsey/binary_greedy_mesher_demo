use criterion::{criterion_group, criterion_main, Criterion};
use new_voxel_testing::{constants::CHUNK_SIZE3, utils::index_to_ivec3};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("index_to_vec3", |b| {
        b.iter(
            || {
                for j in 0..CHUNK_SIZE3 {
                    index_to_ivec3(j);
                }
            },
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);