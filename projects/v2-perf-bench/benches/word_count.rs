//! criterion 基准：词频统计 朴素版 vs 优化版。
//!
//! 关键点：用 `black_box` 包住输入，**阻止编译器把整个计算优化掉**
//! （否则结果未被使用，可能被常量折叠/死代码消除，量出来全是噪声）。
//! `harness = false` 已在 Cargo.toml 配好，这里用 criterion 自己的入口宏。

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use v2_perf_bench::{word_count_fast, word_count_naive};

/// 构造一段有重复词的文本，规模足够大以拉开两版差距。
fn sample_text() -> String {
    let words = [
        "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog", "rust", "perf",
    ];
    let mut s = String::with_capacity(64 * 1024);
    for i in 0..4000 {
        s.push_str(words[i % words.len()]);
        s.push(' ');
    }
    s
}

fn bench_word_count(c: &mut Criterion) {
    let text = sample_text();
    let mut group = c.benchmark_group("word_count");
    group.bench_function("naive", |b| {
        b.iter(|| word_count_naive(black_box(&text)));
    });
    group.bench_function("fast", |b| {
        b.iter(|| word_count_fast(black_box(&text)));
    });
    group.finish();
}

criterion_group!(benches, bench_word_count);
criterion_main!(benches);
