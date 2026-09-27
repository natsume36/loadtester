//! 命令行参数定义（第 1 周交付）。
//! 采用 Clap derive 宏声明式定义，身份信息、帮助、默认值都由宏生成。

use clap::Parser;

/// 命令行入口参数结构。
#[derive(Debug, Clone, Parser)]
#[command(
    name = "loadtester",
    version,
    about = "Rust HTTP 压测工具：并发、限流、延迟分布统计与结果导出",
    long_about = "支持同步/异步请求、HTTP/1.1 与 HTTP/2、HTTPS、多并发、QPS 令牌桶限流、\
                  数据集驱动随机化，并输出 p50/p95/p99、QPS、错误率（console/json/csv）"
)]
pub struct Cli {
    /// 压测目标 URL（也可在配置文件中通过 url 字段提供；两者都缺失则报错）
    #[arg(value_name = "URL")]
    pub url: Option<String>,

    /// 并发数（Worker Pool 的大小，默认 1）
    #[arg(short = 'C', long, value_name = "N")]
    pub concurrency: Option<usize>,

    /// 总请求数（与 --duration 二选一；都不填则默认打满 10000 个请求）
    #[arg(short = 'n', long)]
    pub requests: Option<u64>,

    /// 压测时长（秒）
    #[arg(long)]
    pub duration: Option<u64>,

    /// HTTP 方法（默认 GET；GET/POST/PUT/DELETE/…，支持自定义方法）
    #[arg(short = 'X', long, value_name = "METHOD")]
    pub method: Option<String>,

    /// 请求体（字面字符串，或 "@文件路径"）
    #[arg(short = 'd', long)]
    pub data: Option<String>,

    /// 自定义请求头，格式 "Key: Value"；可重复传入多个（如 -H "Authorization: Bearer x" -H "Content-Type: application/json"）
    #[arg(short = 'H', long = "header", value_name = "HEADER")]
    pub headers: Vec<String>,

    /// 协议版本（默认 http/1.1）：http/1.1 | http/2
    #[arg(long, value_name = "VERSION")]
    pub protocol: Option<String>,

    /// 单请求超时秒数（默认 30）
    #[arg(long, value_name = "SECS")]
    pub timeout: Option<u64>,

    /// 网络失败/超时后的重试次数（默认 0，即不重试）
    #[arg(long, value_name = "N")]
    pub retries: Option<u32>,

    /// TCP/空闲连接保活秒数（默认 30）
    #[arg(long, value_name = "SECS")]
    pub keepalive: Option<u64>,

    /// QPS 限流（令牌桶），不填则不限流
    #[arg(long)]
    pub qps: Option<u64>,

    /// 数据集 JSON 文件；每条记录可覆盖本次请求的 method/url/headers/body
    #[arg(long)]
    pub dataset: Option<String>,

    /// 跳过 TLS 证书校验（自签名/内网测试用）——B 角色新增
    #[arg(long)]
    pub insecure: bool,

    /// 自定义根证书 PEM 文件路径（B 角色新增）
    #[arg(long)]
    pub cacert: Option<String>,

    /// 输出格式（默认 console）：console | json | csv
    #[arg(long)]
    pub format: Option<String>,

    /// 输出文件路径（json/csv 时必填）
    #[arg(short = 'o', long)]
    pub output: Option<String>,

    /// YAML 配置文件路径；CLI 显式参数优先
    #[arg(short = 'c', long)]
    pub config: Option<String>,

    /// p99 延迟阈值（毫秒）；超阈值时进程以退出码 2 退出，便于 CI 流水线判断
    #[arg(long, value_name = "MS")]
    pub p99_threshold: Option<u64>,
}

impl Cli {
    /// 是否指定了结束条件（请求数或时长）。
    #[allow(dead_code)] // 第 2 周实现时长模式时接入
    pub fn has_termination(&self) -> bool {
        self.requests.is_some() || self.duration.is_some()
    }
}