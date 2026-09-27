//! 配置层（config.rs）：YAML 文件 + CLI 参数合并，得到运行时唯一配置。
//! 合并规则：CLI 上显式给出的参数覆盖配置文件；两者都缺省时用默认值。

use crate::cli::Cli;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// HTTP 协议版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http1,
    Http2,
}

/// 输出格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Console,
    Json,
    Csv,
}

/// 从 YAML 文件解析出来的原始配置（全部可选，便于和 CLI 合并）。
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FileConfig {
    pub url: Option<String>,
    pub concurrency: Option<usize>,
    pub requests: Option<u64>,
    pub duration: Option<u64>,
    pub method: Option<String>,
    pub protocol: Option<String>,
    pub qps: Option<u64>,
    pub dataset: Option<String>,
    pub format: Option<String>,
    pub output: Option<String>,
    /// 请求体：字面字符串，或 "@文件路径"（也接受别名 body）
    #[serde(default, alias = "body")]
    pub data: Option<String>,
    /// 自定义请求头，元素格式与 CLI 一致："Key: Value"
    #[serde(default)]
    pub headers: Option<Vec<String>>,
    /// 跳过 TLS 证书校验
    #[serde(default)]
    pub insecure: Option<bool>,
    /// 自定义根证书 PEM 路径
    #[serde(default)]
    pub cacert: Option<String>,
    /// 单请求超时（秒）
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// 网络失败重试次数
    #[serde(default)]
    pub max_retries: Option<u32>,
    /// TCP/空闲连接保活（秒）
    #[serde(default)]
    pub keepalive_secs: Option<u64>,
    /// p99 延迟阈值（毫秒）；超阈值时进程退出码为 2，便于 CI 判断
    #[serde(default)]
    pub p99_threshold_ms: Option<u64>,
}

/// 合并后的运行时配置（CLI > YAML > 默认值），负责把字符串规范化为枚举。
#[derive(Debug, Clone)]
pub struct Config {
    pub url: String,
    pub concurrency: usize,
    pub requests: Option<u64>,
    pub duration: Option<u64>,
    pub method: String,
    pub protocol: Protocol,
    pub qps: Option<u64>,
    pub dataset: Option<String>,
    pub format: OutputFormat,
    pub output: Option<String>,
    /// 跳过 TLS 证书校验（CLI: --insecure / YAML: insecure）
    pub insecure: bool,
    /// 自定义根证书 PEM 路径（CLI: --cacert / YAML: cacert）
    pub cacert: Option<String>,
    /// TCP Keep-Alive / 空闲连接池保留时长（秒）
    pub keepalive_secs: u64,
    /// 单请求超时（秒）
    pub timeout_secs: u64,
    /// 网络失败/超时重试次数
    pub max_retries: u32,
    /// 请求体：字面字符串或 @文件 的内容
    pub body: Option<String>,
    /// 全局自定义请求头（CLI: --header / YAML: headers，对每个请求生效）
    pub headers: Vec<(String, String)>,
    /// p99 延迟阈值（毫秒）；超过时进程以退出码 2 退出，便于 CI 流水线判断
    pub p99_threshold_ms: Option<u64>,
}

impl Config {
    /// 从 CLI 构建配置，读取 YAML 并做合并。
    /// 合并优先级：CLI 显式值 > 配置文件 > 默认值。
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let file = if let Some(path) = cli.config.as_deref() {
            Some(load_file(path)?)
        } else {
            None
        };

        // 目标 URL：CLI 优先，否则取配置文件；两者都缺则报错。
        let url = cli
            .url
            .clone()
            .or_else(|| file.as_ref().and_then(|f| f.url.clone()))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "缺少目标 URL：请在命令行提供 <URL>，或在配置文件中设置 url 字段（可用 -c 指定配置文件）"
                )
            })?;

        let f = file.as_ref();

        // 有内置默认值的字段：CLI 显式值 > 配置文件 > 默认值。
        let concurrency = cli
            .concurrency
            .or_else(|| f.and_then(|c| c.concurrency))
            .unwrap_or(1);
        let method = cli
            .method
            .clone()
            .or_else(|| f.and_then(|c| c.method.clone()))
            .unwrap_or_else(|| "GET".to_string());
        let protocol = cli
            .protocol
            .clone()
            .or_else(|| f.and_then(|c| c.protocol.clone()))
            .unwrap_or_else(|| "http/1.1".to_string());
        let format = cli
            .format
            .clone()
            .or_else(|| f.and_then(|c| c.format.clone()))
            .unwrap_or_else(|| "console".to_string());
        let timeout_secs = cli
            .timeout
            .or_else(|| f.and_then(|c| c.timeout_secs))
            .unwrap_or(30);
        let max_retries = cli
            .retries
            .or_else(|| f.and_then(|c| c.max_retries))
            .unwrap_or(0);
        let keepalive_secs = cli
            .keepalive
            .or_else(|| f.and_then(|c| c.keepalive_secs))
            .unwrap_or(30);

        // 无内置默认值的字段：CLI > 文件 > None。
        let qps = cli.qps.or_else(|| f.and_then(|c| c.qps));
        let requests = cli.requests.or_else(|| f.and_then(|c| c.requests));
        let duration = cli.duration.or_else(|| f.and_then(|c| c.duration));
        let dataset = cli
            .dataset
            .clone()
            .or_else(|| f.and_then(|c| c.dataset.clone()));
        let output = cli
            .output
            .clone()
            .or_else(|| f.and_then(|c| c.output.clone()));
        let cacert = cli
            .cacert
            .clone()
            .or_else(|| f.and_then(|c| c.cacert.clone()));
        let p99_threshold_ms = cli
            .p99_threshold
            .or_else(|| f.and_then(|c| c.p99_threshold_ms));

        // 布尔开关：CLI 或配置文件任一置 true 即生效。
        let insecure = cli.insecure || f.and_then(|c| c.insecure).unwrap_or(false);

        // 请求体：CLI -d 优先，否则取配置文件的 data/body，支持 @文件。
        let raw_data = cli
            .data
            .clone()
            .or_else(|| f.and_then(|c| c.data.clone()));
        let body = read_body(raw_data.as_deref())?;

        // 请求头：配置文件的在前、CLI 的在后；parse_headers 对同名头保留后者（CLI 优先）。
        let mut header_strings: Vec<String> =
            f.and_then(|c| c.headers.clone()).unwrap_or_default();
        header_strings.extend(cli.headers.clone());
        let headers = parse_headers(&header_strings)?;

        Ok(Self {
            url,
            concurrency,
            requests,
            duration,
            method,
            protocol: Protocol::from_str(&protocol)?,
            qps,
            dataset,
            format: OutputFormat::from_str(&format)?,
            output,
            insecure,
            cacert,
            keepalive_secs,
            timeout_secs,
            max_retries,
            body,
            headers,
            p99_threshold_ms,
        })
    }

    /// 无结束条件时的兜底请求数（避免无限压测）。
    pub const DEFAULT_TOTAL: u64 = 10_000;

    /// 估算要派发的总请求数。结束条件二选一：先看请求数，再看时长。
    pub fn effective_total(&self) -> u64 {
        self.requests.unwrap_or(Self::DEFAULT_TOTAL)
    }
}

impl Protocol {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "http/1.1" | "http1" | "http/1" | "h1" => Ok(Protocol::Http1),
            "http/2" | "http2" | "h2" => Ok(Protocol::Http2),
            other => anyhow::bail!("不支持的协议: {other}（可选: http/1.1 或 http/2）"),
        }
    }
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "console" => Ok(OutputFormat::Console),
            "json" => Ok(OutputFormat::Json),
            "csv" => Ok(OutputFormat::Csv),
            other => anyhow::bail!("不支持的输出格式: {other}（可选: console/json/csv）"),
        }
    }
}

/// 读取 YAML 配置文件。
fn load_file(path: &str) -> Result<FileConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("读取配置文件失败: {path}"))?;
    let cfg: FileConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("解析配置文件失败: {path}"))?;
    Ok(cfg)
}

/// 解析 `Key: Value` 形式的请求头列表；值允许包含冒号（按第一个冒号切分）。
/// 同名头（大小写不敏感）保留最后一次出现的值——调用方按"配置文件在前、CLI 在后"
/// 的顺序传入，即可实现 CLI 覆盖配置文件。
fn parse_headers(raw: &[String]) -> Result<Vec<(String, String)>> {
    let mut headers: Vec<(String, String)> = Vec::with_capacity(raw.len());
    for h in raw {
        let (name, value) = h
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("请求头格式错误（应为 'Key: Value'）: {h}"))?;
        let name = name.trim();
        let value = value.trim();
        if name.is_empty() {
            anyhow::bail!("请求头名称不能为空: {h}");
        }
        match headers
            .iter_mut()
            .find(|(existing, _)| existing.eq_ignore_ascii_case(name))
        {
            Some(slot) => slot.1 = value.to_string(),
            None => headers.push((name.to_string(), value.to_string())),
        }
    }
    Ok(headers)
}

/// 解析请求体：字面字符串，或 `@path` 表示从文件读取。
fn read_body(data: Option<&str>) -> Result<Option<String>> {
    match data {
        None => Ok(None),
        Some(s) if s.starts_with('@') => {
            let path = &s[1..];
            let body = std::fs::read_to_string(path)
                .with_context(|| format!("读取请求体文件失败: {path}"))?;
            Ok(Some(body))
        }
        Some(s) => Ok(Some(s.to_string())),
    }
}

/// 校验运行时配置合法性。
pub fn validate(cfg: &Config) -> Result<()> {
    if cfg.concurrency == 0 {
        anyhow::bail!("并发数必须 >= 1（当前为 0）");
    }
    if cfg.url.trim().is_empty() {
        anyhow::bail!("目标 URL 不能为空");
    }
    let lower = cfg.url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        anyhow::bail!("URL 必须以 http:// 或 https:// 开头: {}", cfg.url);
    }
    if cfg.requests.is_some() && cfg.duration.is_some() {
        anyhow::bail!("--requests 与 --duration 不能同时指定，请二选一");
    }
    if cfg.timeout_secs == 0 {
        anyhow::bail!("请求超时必须 >= 1 秒（当前为 0）");
    }
    if cfg.keepalive_secs == 0 {
        anyhow::bail!("连接保活时长必须 >= 1 秒（当前为 0）");
    }
    // 若两者都未指定，则走默认请求数（effective_total），不会无限运行。
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_headers_ok() {
        let raw = vec![
            "Authorization: Bearer abc:123".to_string(), // 值中含冒号也保留
            "  Content-Type : application/json ".to_string(),
        ];
        let hs = parse_headers(&raw).unwrap();
        assert_eq!(hs.len(), 2);
        assert_eq!(hs[0], ("Authorization".to_string(), "Bearer abc:123".to_string()));
        assert_eq!(hs[1], ("Content-Type".to_string(), "application/json".to_string()));
    }

    #[test]
    fn parse_headers_rejects_bad_format() {
        assert!(parse_headers(&["NoColon".to_string()]).is_err());
        assert!(parse_headers(&[" : value".to_string()]).is_err());
    }

    #[test]
    fn parse_headers_last_one_wins_case_insensitive() {
        let raw = vec![
            "X-Trace: from-yaml".to_string(),
            "x-trace: from-cli".to_string(), // 同名（小写）应覆盖
        ];
        let hs = parse_headers(&raw).unwrap();
        assert_eq!(hs.len(), 1);
        assert_eq!(hs[0].0, "X-Trace");
        assert_eq!(hs[0].1, "from-cli");
    }

    /// 构造一个全部字段为缺省的 Cli，测试里按需覆盖。
    fn empty_cli() -> Cli {
        Cli {
            url: None,
            concurrency: None,
            requests: None,
            duration: None,
            method: None,
            data: None,
            headers: Vec::new(),
            protocol: None,
            timeout: None,
            retries: None,
            keepalive: None,
            qps: None,
            dataset: None,
            insecure: false,
            cacert: None,
            format: None,
            output: None,
            config: None,
            p99_threshold: None,
        }
    }

    /// 把 YAML 内容写入临时文件，返回其路径（测试结束自动删除）。
    fn write_temp_yaml(name: &str, content: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("loadtester_cfg_{name}_{}.yaml", std::process::id()));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn yaml_values_used_when_cli_absent() {
        let yaml = "\
url: http://yaml.example.com/
method: POST
protocol: http/2
concurrency: 7
format: json
timeout_secs: 12
max_retries: 3
keepalive_secs: 45
insecure: true
data: '{\"k\":2}'
headers:
  - \"X-From-Yaml: yes\"
";
        let path = write_temp_yaml("yaml_only", yaml);
        let mut cli = empty_cli();
        cli.config = Some(path.to_string_lossy().to_string());

        let cfg = Config::from_cli(&cli).unwrap();
        assert_eq!(cfg.url, "http://yaml.example.com/");
        assert_eq!(cfg.method, "POST");
        assert_eq!(cfg.protocol, Protocol::Http2);
        assert_eq!(cfg.concurrency, 7);
        assert_eq!(cfg.format, OutputFormat::Json);
        assert_eq!(cfg.timeout_secs, 12);
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.keepalive_secs, 45);
        assert!(cfg.insecure);
        assert_eq!(cfg.body.as_deref(), Some("{\"k\":2}"));
        assert_eq!(cfg.headers, vec![("X-From-Yaml".to_string(), "yes".to_string())]);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn cli_explicit_values_override_yaml() {
        let yaml = "\
url: http://yaml.example.com/
method: POST
concurrency: 7
headers:
  - \"X-Trace: from-yaml\"
  - \"X-Only-Yaml: keep\"
";
        let path = write_temp_yaml("cli_override", yaml);
        let mut cli = empty_cli();
        cli.config = Some(path.to_string_lossy().to_string());
        cli.url = Some("http://cli.example.com/".to_string());
        cli.concurrency = Some(3);
        cli.method = Some("PUT".to_string());
        cli.headers = vec!["x-trace: from-cli".to_string()];

        let cfg = Config::from_cli(&cli).unwrap();
        assert_eq!(cfg.url, "http://cli.example.com/");
        assert_eq!(cfg.concurrency, 3, "CLI 并发数必须覆盖 YAML");
        assert_eq!(cfg.method, "PUT", "CLI 方法必须覆盖 YAML");
        // 同名头 CLI 覆盖，YAML 独有的头保留
        let hmap: std::collections::BTreeMap<_, _> =
            cfg.headers.iter().map(|(k, v)| (k.to_lowercase(), v.clone())).collect();
        assert_eq!(hmap.get("x-trace").map(String::as_str), Some("from-cli"));
        assert_eq!(hmap.get("x-only-yaml").map(String::as_str), Some("keep"));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn builtin_defaults_apply_without_yaml() {
        let mut cli = empty_cli();
        cli.url = Some("http://example.com/".to_string());
        let cfg = Config::from_cli(&cli).unwrap();
        assert_eq!(cfg.concurrency, 1);
        assert_eq!(cfg.method, "GET");
        assert_eq!(cfg.protocol, Protocol::Http1);
        assert_eq!(cfg.format, OutputFormat::Console);
        assert_eq!(cfg.timeout_secs, 30);
        assert_eq!(cfg.max_retries, 0);
        assert!(cfg.headers.is_empty());
        assert!(cfg.body.is_none());
    }

    /// 验证 p99 阈值的三层合并优先级：CLI > YAML > 默认 None。
    #[test]
    fn p99_threshold_merge_priority() {
        // 1) 默认无 YAML 且 CLI 未传 -> None
        let mut cli = empty_cli();
        cli.url = Some("http://example.com/".to_string());
        assert!(Config::from_cli(&cli).unwrap().p99_threshold_ms.is_none());

        // 2) 仅 YAML 提供 -> 取 YAML 值
        let yaml = "url: http://yaml.example/\np99_threshold_ms: 500\n";
        let path = write_temp_yaml("p99_yaml", yaml);
        let mut cli = empty_cli();
        cli.config = Some(path.to_string_lossy().to_string());
        assert_eq!(
            Config::from_cli(&cli).unwrap().p99_threshold_ms,
            Some(500)
        );

        // 3) CLI 与 YAML 同时给出 -> CLI 覆盖
        let yaml = "url: http://yaml.example/\np99_threshold_ms: 500\n";
        let path = write_temp_yaml("p99_override", yaml);
        let mut cli = empty_cli();
        cli.config = Some(path.to_string_lossy().to_string());
        cli.p99_threshold = Some(120);
        assert_eq!(
            Config::from_cli(&cli).unwrap().p99_threshold_ms,
            Some(120),
            "CLI --p99-threshold 必须覆盖 YAML"
        );

        std::fs::remove_file(&path).unwrap();
    }
}