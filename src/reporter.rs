//! 输出导出层（C 角色交付）。
//! 按 OutputFormat 输出：控制台（含 min/max/mean、p50/p90/p95/p99/p99.9、
//! 状态码分布、总字节数、错误 TopN）、JSON（含原始样本）、CSV 汇总。

use crate::config::{Config, OutputFormat};
use crate::stats::{Sample, Stats, Summary};
use anyhow::Result;
use std::io::Write;
use std::time::Duration;

/// 纳秒 -> 毫秒（整数），输出阶段统一使用。
fn ns_to_ms(ns: u64) -> u64 {
    ns / 1_000_000
}

/// 纳秒 -> 毫秒（浮点），用于 mean 等需要小数精度的字段。
fn ns_to_ms_f(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

/// 把字节数格式化为人类可读形式（如 1.2 MB、340 KB）。
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.2} GB ({} bytes)", bytes as f64 / GB as f64, bytes)
    } else if bytes >= MB {
        format!("{:.2} MB ({} bytes)", bytes as f64 / MB as f64, bytes)
    } else if bytes >= KB {
        format!("{:.2} KB ({} bytes)", bytes as f64 / KB as f64, bytes)
    } else {
        format!("{} bytes", bytes)
    }
}

/// 为一次完整压测生成报告。
pub fn report(config: &Config, stats: &Stats, elapsed: Duration) -> Result<()> {
    let summary = stats.summarize(elapsed.as_secs_f64());
    let samples = stats.samples();
    match config.format {
        OutputFormat::Console => print_console(&summary),
        OutputFormat::Json => export_json(&summary, &samples, config.output.as_deref()),
        OutputFormat::Csv => export_csv(&summary, config.output.as_deref()),
    }
}

/// 控制台输出。
fn print_console(summary: &Summary) -> Result<()> {
    let out = std::io::stdout();
    let mut o = out.lock();
    writeln!(o, "----- 压测汇总 -----")?;
    writeln!(o, "总请求数   : {}", summary.total)?;
    writeln!(o, "QPS        : {:.1}", summary.qps)?;
    writeln!(
        o,
        "错误数     : {} ({:.2}%)",
        summary.errors,
        summary.error_rate * 100.0
    )?;
    writeln!(
        o,
        "延迟 (ms)  : min={} mean={:.1} max={} p50={} p90={} p95={} p99={} p99.9={}",
        ns_to_ms(summary.min_ns),
        ns_to_ms_f(summary.mean_ns as u64),
        ns_to_ms(summary.max_ns),
        ns_to_ms(summary.p50_ns),
        ns_to_ms(summary.p90_ns),
        ns_to_ms(summary.p95_ns),
        ns_to_ms(summary.p99_ns),
        ns_to_ms(summary.p999_ns),
    )?;
    writeln!(o, "总字节数   : {}", format_bytes(summary.total_bytes))?;
    if !summary.status_code_distribution.is_empty() {
        let parts: String = summary
            .status_code_distribution
            .iter()
            .map(|(code, cnt)| format!("{code}={cnt}"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(o, "状态码分布 : {parts}")?;
    }
    if !summary.top_errors.is_empty() {
        writeln!(o, "错误 TopN   :")?;
        for (msg, cnt) in &summary.top_errors {
            let truncated: String = msg.chars().take(120).collect();
            writeln!(o, "  x{cnt:<6} {truncated}")?;
        }
    }
    Ok(())
}

/// 导出 JSON：汇总 + 状态码分布 + 错误 TopN + 原始样本数组。
fn export_json(summary: &Summary, samples: &[Sample], path: Option<&str>) -> Result<()> {
    let top_errors: Vec<serde_json::Value> = summary
        .top_errors
        .iter()
        .map(|(msg, cnt)| serde_json::json!({ "message": msg, "count": cnt }))
        .collect();
    let sample_json: Vec<serde_json::Value> = samples
        .iter()
        .map(|s| {
            serde_json::json!({
                "status_code": s.status_code,
                "elapsed_ms": ns_to_ms_f(s.elapsed_ns),
                "bytes": s.bytes,
                "http_version": s.http_version,
            })
        })
        .collect();

    // 状态码分布：JSON 对象键必须是字符串，这里用数字字符串作为 key
    let mut status_dist = serde_json::Map::new();
    for (code, cnt) in &summary.status_code_distribution {
        status_dist.insert(code.to_string(), serde_json::json!(cnt));
    }

    let json = serde_json::json!({
        "summary": {
            "total": summary.total,
            "errors": summary.errors,
            "qps": summary.qps,
            "error_rate": summary.error_rate,
            "total_bytes": summary.total_bytes,
            "latency_ms": {
                "min": ns_to_ms_f(summary.min_ns),
                "mean": ns_to_ms_f(summary.mean_ns as u64),
                "max": ns_to_ms_f(summary.max_ns),
                "p50": ns_to_ms_f(summary.p50_ns),
                "p90": ns_to_ms_f(summary.p90_ns),
                "p95": ns_to_ms_f(summary.p95_ns),
                "p99": ns_to_ms_f(summary.p99_ns),
                "p99.9": ns_to_ms_f(summary.p999_ns),
            },
            "status_code_distribution": status_dist,
        },
        "top_errors": top_errors,
        "samples": sample_json,
    });
    let content = serde_json::to_string_pretty(&json)?;
    write_output(path, content.as_bytes())
}

/// 导出 CSV：汇总表（原始样本量可能很大，故仅汇总）。
fn export_csv(summary: &Summary, path: Option<&str>) -> Result<()> {
    let header = "total,errors,qps,error_rate,min_ms,mean_ms,max_ms,p50_ms,p90_ms,p95_ms,p99_ms,p999_ms,total_bytes";
    let row = format!(
        "{},{},{:.1},{:.4},{:.3},{:.3},{:.3},{},{},{},{},{},{}",
        summary.total,
        summary.errors,
        summary.qps,
        summary.error_rate,
        ns_to_ms_f(summary.min_ns),
        ns_to_ms_f(summary.mean_ns as u64),
        ns_to_ms_f(summary.max_ns),
        ns_to_ms(summary.p50_ns),
        ns_to_ms(summary.p90_ns),
        ns_to_ms(summary.p95_ns),
        ns_to_ms(summary.p99_ns),
        ns_to_ms(summary.p999_ns),
        summary.total_bytes,
    );
    let content = format!("{header}\n{row}\n");
    write_output(path, content.as_bytes())
}

/// 写文件（给定路径）或 stdout（无路径）。
fn write_output(path: Option<&str>, bytes: &[u8]) -> Result<()> {
    match path {
        Some(p) => {
            let mut f = std::fs::File::create(p)?;
            f.write_all(bytes)?;
            Ok(())
        }
        None => {
            let out = std::io::stdout();
            let mut o = out.lock();
            o.write_all(bytes)?;
            o.flush()?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Stats;

    fn fake_summary() -> Summary {
        let stats = Stats::new();
        stats.record(200, 1_500_000, "HTTP/1.1".into(), 1024);
        stats.record(200, 3_000_000, "HTTP/1.1".into(), 2048);
        stats.record(404, 5_000_000, "HTTP/1.1".into(), 256);
        stats.summarize(1.0)
    }

    #[test]
    fn console_output_includes_new_metrics() {
        let s = fake_summary();
        // 临时文件路径不传，让控制台输出落到 stdout；这里只校验内部构造不报错
        print_console(&s).unwrap();
    }

    #[test]
    fn json_export_contains_p90_and_distribution() {
        let s = fake_summary();
        let samples = vec![];
        // 直接复用 export_json 的输出语义：写到一个临时路径
        let tmp = std::env::temp_dir().join("reporter_test.json");
        export_json(&s, &samples, tmp.to_str()).unwrap();
        let content = std::fs::read_to_string(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        assert!(content.contains("p90"), "JSON 应包含 p90 字段");
        assert!(content.contains("p99.9"), "JSON 应包含 p99.9 字段");
        assert!(content.contains("status_code_distribution"));
        assert!(content.contains("total_bytes"));
        assert!(content.contains("\"200\"") && content.contains("\"404\""));
    }

    #[test]
    fn csv_header_has_all_percentiles_and_bytes() {
        let s = fake_summary();
        let tmp = std::env::temp_dir().join("reporter_test.csv");
        export_csv(&s, tmp.to_str()).unwrap();
        let content = std::fs::read_to_string(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        let header_line = content.lines().next().unwrap();
        for field in [
            "p50_ms", "p90_ms", "p95_ms", "p99_ms", "p999_ms", "total_bytes",
        ] {
            assert!(header_line.contains(field), "CSV header 缺少字段 {field}");
        }
    }

    #[test]
    fn format_bytes_picks_reasonable_unit() {
        assert_eq!(format_bytes(512), "512 bytes");
        assert!(format_bytes(2048).contains("KB"));
        assert!(format_bytes(2_000_000).contains("MB"));
        assert!(format_bytes(3_000_000_000).contains("GB"));
    }
}
