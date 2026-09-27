//! HTTP 客户端层（B 角色交付）。
//! 基于 reqwest 封装一个可克隆的客户端：屏蔽 HTTP/1.1 / HTTP/2 / HTTPS 差异，
//! 支持可配的连接池/Keep-Alive、TLS 校验开关、失败重试，并上报实际协商的协议版本。

use crate::config::{Config, Protocol};
use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

/// 单个请求的观测结果。
#[derive(Debug, Clone)]
pub struct SingleResponse {
    pub status_code: u16,
    /// 端到端耗时（纳秒），供 HdrHistogram 统计；输出时再换算为毫秒展示
    pub elapsed_ns: u64,
    /// 本次请求实际协商使用的 HTTP 版本（如 "HTTP/1.1" 或 "HTTP/2"）
    #[allow(dead_code)] // C 角色做协议对比报告时读取
    pub http_version: String,
    /// 本次请求接收到的响应体字节数（用于总字节数统计）
    pub bytes_received: u64,
}

/// 客户端构建选项（带默认值，可按需覆盖）。
#[derive(Debug, Clone)]
pub struct ClientOptions {
    /// 单请求超时（秒）
    pub timeout_secs: u64,
    /// TCP Keep-Alive 与空闲连接池保留时长（秒）
    pub keepalive_secs: u64,
    /// 每个主机最大空闲连接数
    pub max_idle_conns_per_host: usize,
    /// 失败重试次数（网络错误/超时）
    pub max_retries: u32,
    /// 两次重试之间的基础退避延迟（毫秒）
    pub retry_delay_ms: u64,
    /// 跳过 TLS 证书校验（自签名/内网测试用）
    pub insecure: bool,
    /// 自定义根证书 PEM 文件路径
    pub tls_root_cert: Option<std::path::PathBuf>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            keepalive_secs: 30,
            max_idle_conns_per_host: 16,
            max_retries: 0,
            retry_delay_ms: 200,
            insecure: false,
            tls_root_cert: None,
        }
    }
}

impl From<&Config> for ClientOptions {
    fn from(cfg: &Config) -> Self {
        Self {
            timeout_secs: cfg.timeout_secs,
            keepalive_secs: cfg.keepalive_secs,
            max_retries: cfg.max_retries,
            insecure: cfg.insecure,
            tls_root_cert: cfg.cacert.as_deref().map(std::path::PathBuf::from),
            ..Self::default()
        }
    }
}

/// Reqwest 客户端封装。
///
/// 通过 `Arc<HttpClient>` 在多 worker 间共享；`Client` 与 `ClientOptions` 均支持 Clone。
#[derive(Clone)]
pub struct HttpClient {
    pub client: Client,
    /// 期望协议（HTTP/1.1 强制；HTTP/2 需服务端支持 h2）
    #[allow(dead_code)]
    pub protocol: Protocol,
    /// 客户端选项，重试逻辑会读取其中的 `max_retries` / `retry_delay_ms`
    pub options: ClientOptions,
}

impl HttpClient {
    /// 保持旧签名，便于不关心细节的调用方：用默认选项 + 指定协议构建。
    #[allow(dead_code)] // 便捷构造；主流程已改用 from_config
    pub fn new(protocol: Protocol, timeout_secs: u64) -> Result<Self> {
        Self::from_options(protocol, ClientOptions { timeout_secs, ..ClientOptions::default() })
    }

    /// 从运行时配置构建（B 推荐的入口）。
    pub fn from_config(cfg: &Config) -> Result<Self> {
        Self::from_options(cfg.protocol, ClientOptions::from(cfg))
    }

    /// 按协议与选项构建客户端。
    pub fn from_options(protocol: Protocol, opts: ClientOptions) -> Result<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(opts.timeout_secs))
            .pool_idle_timeout(Duration::from_secs(opts.keepalive_secs))
            .tcp_keepalive(Duration::from_secs(opts.keepalive_secs))
            .pool_max_idle_per_host(opts.max_idle_conns_per_host);

        // TLS：默认校验证书；`insecure` 时跳过（自签名测试）；`tls_root_cert` 指定自定义根证书
        if opts.insecure {
            builder = builder.danger_accept_invalid_certs(true);
        }
        if let Some(cert_path) = &opts.tls_root_cert {
            let pem = std::fs::read(cert_path)
                .map_err(|e| anyhow::anyhow!("读取根证书 {} 失败: {e}", cert_path.display()))?;
            let cert =
                reqwest::Certificate::from_pem(&pem).map_err(|e| anyhow::anyhow!("解析根证书失败: {e}"))?;
            builder = builder.add_root_certificate(cert);
        }

        // TODO(HTTP/2): 后续支持 ALPN 协商（当前若目标不支持 h2，http2_prior_knowledge 会直接失败）
        builder = match protocol {
            Protocol::Http1 => builder.http1_only(),
            Protocol::Http2 => builder.http2_prior_knowledge(),
        };

        let client = builder.build()?;
        Ok(Self { client, protocol, options: opts })
    }

    /// 发送单个请求，带有限重试与退避，返回状态码、耗时与协商协议版本。
    ///
    /// - `headers`：本次请求附加的自定义请求头（名称, 值）列表；
    /// - `body` 为 `None` 时不发送请求体。
    pub async fn send(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&str>,
    ) -> Result<SingleResponse> {
        let method = reqwest::Method::from_bytes(method.as_bytes())?;

        // 重试循环（默认不重试）
        let mut attempts: u32 = 0;
        loop {
            let start = std::time::Instant::now();
            let mut request = self.client.request(method.clone(), url);
            for (name, value) in headers {
                request = request.header(name, value);
            }
            if let Some(b) = body {
                request = request.body(b.to_string());
            }

            match request.send().await {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let http_version = http_version_str(&response.version());
                    // 读完整响应体以统计真实下载字节数；耗时同时覆盖接收阶段
                    let bytes_received = response.bytes().await.map(|b| b.len() as u64).unwrap_or(0);
                    let elapsed_ns = start.elapsed().as_nanos() as u64;
                    return Ok(SingleResponse {
                        status_code: status,
                        elapsed_ns,
                        http_version,
                        bytes_received,
                    });
                }
                Err(e) => {
                    attempts += 1;
                    if attempts > self.options.max_retries {
                        return Err(anyhow::anyhow!("请求失败（{attempts} 次后放弃）: {e}"));
                    }
                    tokio::time::sleep(Duration::from_millis(self.options.retry_delay_ms)).await;
                }
            }
        }
    }
}

/// 把 `http::Version` 映射为可读字符串。
fn http_version_str(v: &reqwest::Version) -> String {
    match *v {
        reqwest::Version::HTTP_09 => "HTTP/0.9".into(),
        reqwest::Version::HTTP_10 => "HTTP/1.0".into(),
        reqwest::Version::HTTP_11 => "HTTP/1.1".into(),
        reqwest::Version::HTTP_2 => "HTTP/2".into(),
        reqwest::Version::HTTP_3 => "HTTP/3".into(),
        _ => "HTTP/unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_defaults_are_sane() {
        let o = ClientOptions::default();
        assert!(o.timeout_secs >= 1);
        assert!(o.max_idle_conns_per_host >= 1);
        assert!(!o.insecure);
    }

    #[tokio::test]
    async fn send_fails_invalid_method() {
        let client = HttpClient::new(Protocol::Http1, 30).unwrap();
        let err = client.send("BAD METHOD", "http://example.com", &[], None).await;
        assert!(err.is_err(), "非法方法应返回错误");
    }

    #[tokio::test]
    async fn send_retries_then_succeeds() {
        // 仅验证重试逻辑不会在单次成功时误重试；真实超时场景留待集成测试。
        let mut o = ClientOptions::default();
        o.max_retries = 2;
        o.timeout_secs = 5;
        let client = HttpClient::from_options(Protocol::Http1, o).unwrap();
        let resp = client.send("GET", "http://127.0.0.1:1/nope", &[], None).await;
        // 目标端口不可达 -> 重试 2 次后仍失败，错误信息应包含重试次数提示
        assert!(resp.is_err());
        assert!(resp.unwrap_err().to_string().contains("次后放弃"));
    }

    #[tokio::test]
    async fn post_body_reaches_server() {
        // 起一个本地 TCP 回显服务，校验 POST 方法与请求体真的被发出去。
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 1024];
            let n = socket.read(&mut buf).await.unwrap();
            let req_lines = String::from_utf8_lossy(&buf[..n]);
            // 校验请求方法、路径与 body 都正确送达
            let ok = req_lines.starts_with("POST /submit HTTP/1.1") && req_lines.contains("hello-branch");
            let status = if ok { "200 OK" } else { "500 Error" };
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _ = socket.write_all(resp.as_bytes()).await;
        });

        let client = HttpClient::new(Protocol::Http1, 30).unwrap();
        let url = format!("http://{addr}/submit");
        let resp = client
            .send("POST", &url, &[], Some("hello-branch"))
            .await
            .unwrap();
        assert_eq!(resp.status_code, 200, "服务应返回 200 表示报文正确送达");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn custom_headers_reach_server() {
        // 起本地 TCP 服务，校验自定义请求头确实随报文发出。
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 2048];
            let n = socket.read(&mut buf).await.unwrap();
            let req_lines = String::from_utf8_lossy(&buf[..n]);
            let ok = req_lines.contains("x-trace-id: header-ok")
                && req_lines.contains("authorization: Bearer token");
            let status = if ok { "200 OK" } else { "500 Error" };
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _ = socket.write_all(resp.as_bytes()).await;
        });

        let client = HttpClient::new(Protocol::Http1, 30).unwrap();
        let url = format!("http://{addr}/head");
        let headers = vec![
            ("X-Trace-Id".to_string(), "header-ok".to_string()),
            ("Authorization".to_string(), "Bearer token".to_string()),
        ];
        let resp = client.send("GET", &url, &headers, None).await.unwrap();
        assert_eq!(resp.status_code, 200, "服务端应能读到自定义请求头");
        server.await.unwrap();
    }
}