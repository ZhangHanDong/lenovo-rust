//! criterion 基准：平方和 顺序版 vs rayon 并行版。
//!
//! 并行不是"永远更快"——线程切分与归约有固定开销，只有计算量足够大时
//! 才回本。基准的价值正是把这个"回本点"量出来，而不是凭直觉拍板。

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use v2_perf_bench::{sum_squares_parallel, sum_squares_sequential};

fn bench_sum_squares(c: &mut Criterion) {
    let values: Vec<u64> = (0..1_000_000u64).collect();
    let mut group = c.benchmark_group("sum_squares");
    group.bench_function("sequential", |b| {
        b.iter(|| sum_squares_sequential(black_box(&values)));
    });
    group.bench_function("parallel", |b| {
        b.iter(|| sum_squares_parallel(black_box(&values)));
    });
    group.finish();
}

criterion_group!(benches, bench_sum_squares);
criterion_main!(benches);
