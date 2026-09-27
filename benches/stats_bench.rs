//! 统计引擎微基准（设计文档七、测试策略）。
//!
//! 目标：测量 `Stats::record` 单条样本写入开销，以及 `summarize` 在 10 万样本下的聚合开销。
//! HdrHistogram 标称记录速度 3-6 ns，本基准用于回归验证。

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use loadtester::stats::Stats;

fn bench_record(c: &mut Criterion) {
    c.bench_function("stats_record_one_sample", |b| {
        let stats = Stats::new();
        b.iter(|| {
            stats.record(
                black_box(200),
                black_box(1_500_000),
                black_box("HTTP/1.1").to_string(),
                black_box(128),
            );
        });
    });
}

fn bench_summarize(c: &mut Criterion) {
    let stats = Stats::new();
    for i in 1..=100_000u64 {
        stats.record(200, i * 1_000, "HTTP/1.1".to_string(), 64);
    }
    c.bench_function("stats_summarize_100k_samples", |b| {
        b.iter(|| stats.summarize(black_box(1.0)));
    });
}

criterion_group!(benches, bench_record, bench_summarize);
criterion_main!(benches);
