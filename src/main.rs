//! 程序入口（第 1 周交付）。
//! 职责：解析 CLI -> 合并配置 -> 构建客户端与统计 -> 调度 Worker -> 输出报告。

use anyhow::Result;
use clap::Parser;
use loadtester::{cli, config, dataset, http_client, reporter, stats, worker};
use std::sync::Arc;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let cfg = config::Config::from_cli(&cli)?;
    config::validate(&cfg)?;

    run(&cfg).await
}

/// 压测主流程：构建组件并按配置调度执行。
async fn run(cfg: &config::Config) -> Result<()> {
    // 1. HTTP 客户端（按协议与选项构建 http/1.1 或 http/2、TLS/Keep-Alive 策略）
    let client = Arc::new(http_client::HttpClient::from_config(cfg)?);

    // 2. 统计引擎
    let stats = Arc::new(stats::Stats::new());

    // 3. 数据集（可选，驱动请求随机化）
    let dataset: Option<Arc<dataset::Dataset>> = match cfg.dataset.as_deref() {
        Some(p) => Some(Arc::new(dataset::Dataset::load(p)?)),
        None => None,
    };

    // 4. 开始压测
    let start = Instant::now();
    let sent = worker::run(cfg, client, stats.clone(), dataset).await?;
    let elapsed = start.elapsed();

    // 5. 输出报告
    reporter::report(cfg, &stats, elapsed)?;

    eprintln!("已派发 {} 个请求", sent);

    // 6. CI 阈值检查：p99 超阈值则以退出码 2 退出，便于流水线判断
    if let Some(threshold_ms) = cfg.p99_threshold_ms {
        let summary = stats.summarize(elapsed.as_secs_f64());
        let p99_ms = summary.p99_ns / 1_000_000;
        if p99_ms > threshold_ms {
            eprintln!(
                "性能阈值未达标：p99 = {}ms > 阈值 {}ms，退出码 2（CI 失败信号）",
                p99_ms, threshold_ms
            );
            std::process::exit(2);
        }
    }

    Ok(())
}
