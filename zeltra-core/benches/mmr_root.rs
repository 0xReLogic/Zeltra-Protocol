use ark_bls12_381::Fr;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use zeltra_core::MerkleMountainRange;

fn bench_mmr_get_root(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmr_root");

    for n in [100, 500, 1000].iter() {
        let mut mmr = MerkleMountainRange::new();
        for i in 0..*n {
            mmr.append(Fr::from(i as u64));
        }

        group.bench_with_input(format!("cached_n={}", n), &mmr, |b, mmr| {
            b.iter(|| black_box(mmr.get_root()))
        });

        group.bench_with_input(format!("uncached_bag_peaks_n={}", n), &mmr, |b, mmr| {
            b.iter(|| {
                black_box(MerkleMountainRange::bag_peaks(
                    &mmr.get_peaks(),
                    mmr.leaf_count,
                ))
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_mmr_get_root);
criterion_main!(benches);
