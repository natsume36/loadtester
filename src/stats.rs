//! 统计聚合引擎（C 角色交付）。
//! HdrHistogram 记录纳秒级延迟分布 + 原子计数器记录总量，
//! 附带状态码分布、总字节数、错误 TopN 与原始样本缓冲。

use hdrhistogram::Histogram;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// 一条原始样本（供分组/原始导出）。
#[derive(Debug, Clone)]
pub struct Sample {
    pub status_code: u16,
    /// 端到端耗时（纳秒）
    pub elapsed_ns: u64,
    pub http_version: String,
    /// 响应体字节数
    pub bytes: u64,
}

/// 汇总统计结果（导出/报告用）。延迟字段以纳秒存储，输出层再换算为毫秒。
#[derive(Debug, Clone, Default)]
pub struct Summary {
    pub total: u64,
    pub errors: u64,
    pub qps: f64,
    pub min_ns: u64,
    pub max_ns: u64,
    pub mean_ns: f64,
    pub p50_ns: u64,
    pub p90_ns: u64,
    pub p95_ns: u64,
    pub p99_ns: u64,
    pub p999_ns: u64,
    pub error_rate: f64,
    /// 错误信息 TopN（消息, 出现次数）
    pub top_errors: Vec<(String, u64)>,
    /// 状态码分布（状态码, 出现次数），按状态码升序
    pub status_code_distribution: BTreeMap<u16, u64>,
    /// 接收到的响应体总字节数
    pub total_bytes: u64,
}

/// 原始样本缓冲上限，避免无界增长吃光内存。
const MAX_SAMPLES: usize = 200_000;

/// 并发安全的统计引擎。
pub struct Stats {
    total: AtomicU64,
    errors: AtomicU64,
    total_elapsed_ns: AtomicU64,
    total_bytes: AtomicU64,
    histogram: Mutex<Histogram<u64>>,
    error_counts: Mutex<BTreeMap<String, u64>>,
    status_code_distribution: Mutex<BTreeMap<u16, u64>>,
    samples: Mutex<Vec<Sample>>,
}

impl Stats {
    /// 创建统计引擎。3 为直方图有效数字（significance），权衡精度/内存。
    /// HdrHistogram<u64>::new(3) 默认可记录整个 u64 范围内的纳秒值（≈584 年），无需自动扩容。
    pub fn new() -> Self {
        let histogram = Histogram::<u64>::new(3).expect("创建直方图失败");
        Self {
            total: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            total_elapsed_ns: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            histogram: Mutex::new(histogram),
            error_counts: Mutex::new(BTreeMap::new()),
            status_code_distribution: Mutex::new(BTreeMap::new()),
            samples: Mutex::new(Vec::new()),
        }
    }

    /// 记录一次带状态码的请求。
    pub fn record(
        &self,
        status_code: u16,
        elapsed_ns: u64,
        http_version: String,
        bytes: u64,
    ) {
        self.total.fetch_add(1, Ordering::Relaxed);
        self.total_elapsed_ns
            .fetch_add(elapsed_ns, Ordering::Relaxed);
        self.total_bytes.fetch_add(bytes, Ordering::Relaxed);
        // TODO: 4xx/5xx 错误口径可配置（如仅 5xx 视为错误）
        if status_code >= 400 {
            self.errors.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut h) = self.histogram.lock() {
            let _ = h.record(elapsed_ns);
        }
        if let Ok(mut m) = self.status_code_distribution.lock() {
            *m.entry(status_code).or_insert(0) += 1;
        }
        self.push_sample(status_code, elapsed_ns, http_version, bytes);
    }

    /// 记录一次请求失败（网络错误/超时等），并归并到错误 TopN。
    pub fn record_error(&self, message: &str) {
        self.total.fetch_add(1, Ordering::Relaxed);
        self.errors.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut map) = self.error_counts.lock() {
            *map.entry(message.to_string()).or_insert(0) += 1;
        }
        // 状态码 0 表示网络层失败（无法获得 HTTP 状态码）
        if let Ok(mut m) = self.status_code_distribution.lock() {
            *m.entry(0).or_insert(0) += 1;
        }
        self.push_sample(0, 0, String::from("-"), 0);
    }

    /// 取 min/max 延时（纳秒）。
    fn range_ns(&self) -> (u64, u64) {
        let ok = self.histogram.lock().ok();
        match ok {
            Some(h) => {
                if h.len() == 0 {
                    (0, 0)
                } else {
                    let min = h.min();
                    let max = h.max();
                    (if min == u64::MAX { 0 } else { min }, max)
                }
            }
            None => (0, 0),
        }
    }

    /// 取 p50 / p90 / p95 / p99 / p99.9 分位数（纳秒）。
    pub fn percentiles_ns(&self) -> (u64, u64, u64, u64, u64) {
        let ok = self.histogram.lock().ok();
        match ok {
            Some(h) => (
                h.value_at_quantile(0.50),
                h.value_at_quantile(0.90),
                h.value_at_quantile(0.95),
                h.value_at_quantile(0.99),
                h.value_at_quantile(0.999),
            ),
            None => (0, 0, 0, 0, 0),
        }
    }

    /// 错误信息 TopN（默认取前 10）。
    pub fn top_errors(&self, n: usize) -> Vec<(String, u64)> {
        let ok = self.error_counts.lock().ok();
        match ok {
            Some(map) => {
                let mut v: Vec<(String, u64)> = map.iter().map(|(k, c)| (k.clone(), *c)).collect();
                v.sort_by(|a, b| b.1.cmp(&a.1));
                v.truncate(n);
                v
            }
            None => Vec::new(),
        }
    }

    /// 状态码分布快照（按状态码升序）。
    pub fn status_code_distribution(&self) -> BTreeMap<u16, u64> {
        self.status_code_distribution
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default()
    }

    /// 累计接收到的总字节数。
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes.load(Ordering::Relaxed)
    }

    /// 复制原始样本（供 JSON 原始导出）。
    pub fn samples(&self) -> Vec<Sample> {
        self.samples.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// 汇总为快照，供报告/导出统一消费。
    pub fn summarize(&self, elapsed_secs: f64) -> Summary {
        let total = self.total.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);
        let (p50, p90, p95, p99, p999) = self.percentiles_ns();
        let (min_ns, max_ns) = self.range_ns();
        let qps = if elapsed_secs > 0.0 {
            total as f64 / elapsed_secs
        } else {
            0.0
        };
        let error_rate = if total > 0 {
            errors as f64 / total as f64
        } else {
            0.0
        };
        let mean_ns = if total > 0 {
            self.total_elapsed_ns.load(Ordering::Relaxed) as f64 / total as f64
        } else {
            0.0
        };
        Summary {
            total,
            errors,
            qps,
            min_ns,
            max_ns,
            mean_ns,
            p50_ns: p50,
            p90_ns: p90,
            p95_ns: p95,
            p99_ns: p99,
            p999_ns: p999,
            error_rate,
            top_errors: self.top_errors(10),
            status_code_distribution: self.status_code_distribution(),
            total_bytes: self.total_bytes(),
        }
    }

    fn push_sample(
        &self,
        status_code: u16,
        elapsed_ns: u64,
        http_version: String,
        bytes: u64,
    ) {
        if let Ok(mut s) = self.samples.lock() {
            if s.len() < MAX_SAMPLES {
                s.push(Sample {
                    status_code,
                    elapsed_ns,
                    http_version,
                    bytes,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_accumulates_total_and_bytes() {
        let stats = Stats::new();
        stats.record(200, 1_000_000, "HTTP/1.1".into(), 128);
        stats.record(200, 2_000_000, "HTTP/1.1".into(), 256);
        stats.record(500, 3_000_000, "HTTP/1.1".into(), 64);
        let s = stats.summarize(0.0);
        assert_eq!(s.total, 3);
        assert_eq!(s.total_bytes, 128 + 256 + 64);
        // 500 视为错误
        assert_eq!(s.errors, 1);
    }

    #[test]
    fn status_code_distribution_counts_each_status() {
        let stats = Stats::new();
        for _ in 0..3 {
            stats.record(200, 1_000_000, "HTTP/1.1".into(), 10);
        }
        for _ in 0..2 {
            stats.record(404, 1_000_000, "HTTP/1.1".into(), 10);
        }
        stats.record(500, 1_000_000, "HTTP/1.1".into(), 10);
        let dist = stats.status_code_distribution();
        assert_eq!(dist.get(&200), Some(&3));
        assert_eq!(dist.get(&404), Some(&2));
        assert_eq!(dist.get(&500), Some(&1));
        assert_eq!(dist.len(), 3);
    }

    #[test]
    fn record_error_uses_status_zero_in_distribution() {
        let stats = Stats::new();
        stats.record_error("connection refused");
        let dist = stats.status_code_distribution();
        assert_eq!(dist.get(&0), Some(&1));
        assert_eq!(stats.summarize(0.0).errors, 1);
    }

    #[test]
    fn percentiles_increase_monotonically() {
        let stats = Stats::new();
        // 注入 100 条递增延迟样本（1ms..100ms，换算为纳秒）
        for i in 1..=100u64 {
            stats.record(200, i * 1_000_000, "HTTP/1.1".into(), 0);
        }
        let s = stats.summarize(0.0);
        assert!(s.p50_ns <= s.p90_ns, "p50 应 <= p90");
        assert!(s.p90_ns <= s.p95_ns, "p90 应 <= p95");
        assert!(s.p95_ns <= s.p99_ns, "p95 应 <= p99");
        assert!(s.p99_ns <= s.p999_ns, "p99 应 <= p99.9");
        // p50 在 1..100 的均匀分布下应靠近 50ms
        let p50_ms = s.p50_ns as f64 / 1_000_000.0;
        assert!(p50_ms >= 40.0 && p50_ms <= 60.0, "p50 应靠近 50ms，实际 {p50_ms}ms");
    }

    #[test]
    fn summarize_handles_empty_stats() {
        let stats = Stats::new();
        let s = stats.summarize(1.0);
        assert_eq!(s.total, 0);
        assert_eq!(s.total_bytes, 0);
        assert!(s.status_code_distribution.is_empty());
        assert_eq!(s.p50_ns, 0);
        assert_eq!(s.p999_ns, 0);
    }

    #[test]
    fn samples_carry_bytes_and_ns_fields() {
        let stats = Stats::new();
        stats.record(200, 7_500_000, "HTTP/2".into(), 1024);
        let v = stats.samples();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].status_code, 200);
        assert_eq!(v[0].elapsed_ns, 7_500_000);
        assert_eq!(v[0].bytes, 1024);
        assert_eq!(v[0].http_version, "HTTP/2");
    }
}
