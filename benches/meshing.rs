use std::sync::Arc;

use bevy::utils::default;
use criterion::{criterion_group, criterion_main, Criterion};
use new_voxel_testing::{
    chunk::ChunkData,
    chunks_refs::ChunksRefs,
    greedy_mesher_optimized,
    lod::Lod,
    voxel::{BlockData, BlockFlags, BlockId, BlockRegistry},
};

/*fn binary_mesh_optimized(chunks_refs: ChunksRefs) {
    let block_registry = Arc::new(BlockRegistry {
        block_flags: vec![BlockFlags::empty(), BlockFlags::SOLID, BlockFlags::SOLID],
        ..default()
    });

    let m = greedy_mesher_optimized::build_chunk_mesh(&chunks_refs, Lod::L32, block_registry, BlockFlags::SOLID, true, false);
}*/

// helper for incrementing and constructing chunksrefs
/*fn make_chunks_refs(s: &mut u64) -> ChunksRefs {
    *s += 1;
    ChunksRefs::make_dummy_chunk_refs(*s)
}*/

fn make_empty() -> ChunksRefs {
    let mut chunks = vec![];
    for _i in 0..3 * 3 * 3 {
        chunks.push(Arc::new(ChunkData {
            voxels: vec![BlockData {
                block_type: BlockId(0),
            }],
        }));
    }
    ChunksRefs { chunks }
}

fn make_filled() -> ChunksRefs {
    let mut chunks = vec![];
    for _i in 0..3 * 3 * 3 {
        chunks.push(Arc::new(ChunkData {
            voxels: vec![BlockData {
                block_type: BlockId(2),
            }],
        }));
    }
    ChunksRefs { chunks }
}

fn slicer(data: [u32; 32]) {
    greedy_mesher_optimized::greedy_mesh_binary_plane(data, 32);
}

fn criterion_benchmark(c: &mut Criterion) {
    // c.bench_function("greedy slicer, 1 plane", |b| {
    //     b.iter_with_setup(
    //         || {
    //             let mut data = [0u32; 32];
    //             let mut rng = rand::thread_rng();
    //             for y in 0..32 {
    //                 for x in 0..32 {
    //                     if rng.gen_range(0..=1) == 0 {
    //                         data[x] |= 1 << y;
    //                     }
    //                 }
    //             }
    //             data
    //         },
    //         |i| slicer(i),
    //     )
    // });
    // c.bench_function("greedy slicer, filled 0", |b| {
    //     b.iter_with_setup(|| [0u32; 32], |i| slicer(i))
    // });
    // c.bench_function("greedy slicer, filled 1", |b| {
    //     b.iter_with_setup(|| [1u32; 32], |i| slicer(i))
    // });

    /*c.bench_function("GREEDY meshing OPTIMIZED: 1 chunk [ao]", |b| {
        let mut s = 0;
        b.iter_with_setup(|| make_chunks_refs(&mut s), binary_mesh_optimized)
    });*/
    // c.bench_function("GREEDY meshing OPTIMIZED: 1 chunk [ao] FILLED", |b| {
    //     b.iter_with_setup(|| make_filled(), |i| binary_mesh_optimized(i))
    // });
    // c.bench_function("GREEDY meshing OPTIMIZED: 1 chunk [ao] EMPTY", |b| {
    //     b.iter_with_setup(|| make_empty(), |i| binary_mesh_optimized(i))
    // });

    // let group = c.benchmark_group("yes");
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
