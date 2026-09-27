//! Worker 模块微基准（设计文档七、测试策略）。
//!
//! 目标：测量 `TokenBucket::acquire` 在不同速率下的放行开销；
//! 高 QPS 下限流是否成为瓶颈（标准要求令牌桶算法真实可用）。

use criterion::{criterion_group, criterion_main, Criterion};
use loadtester::worker::TokenBucket;

fn bench_acquire(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_bucket_acquire");
    for rate in [100u64, 1_000, 10_000, 100_000] {
        group.bench_function(format!("rate_{rate}"), |b| {
            let bucket = TokenBucket::new(rate);
            // 用 tokio 当前线程 runtime 同步驱动 acquire
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("tokio runtime");
            b.iter(|| {
                rt.block_on(async { bucket.acquire().await });
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_acquire);
criterion_main!(benches);
