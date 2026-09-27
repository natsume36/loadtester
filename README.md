# loadtester — Rust HTTP 压测工具

基于 Rust / Tokio 的命令行 HTTP 压测工具：支持高并发、HTTP/1.1 与 HTTP/2、HTTPS、精准 QPS 限流（令牌桶）、数据集驱动随机化，并输出**纳秒精度**延迟分布（p50/p90/p95/p99/p99.9）、状态码分布、总字节数与 QPS/错误率，结果可导出为 console / JSON / CSV，并支持 p99 阈值驱动的 CI 退出码。

实训项目：4 人团队，6 周开发（MVP 骨架已完整可运行）。

---

## 功能特性

- 并发控制：dispatcher + `tokio::sync::Semaphore` + `tokio::task::JoinSet`，把同时在途请求数严格限制在 `--concurrency` 之内（RAII permit 自动归还）
- 请求模式：总请求数（`-n`）或时长（`--duration`）双模式
- 配置方式：命令行参数 + YAML 配置文件（`-c`），CLI 显式参数优先于 YAML
- HTTP 方法：GET/POST/PUT/DELETE 及任意自定义方法
- 请求体：字面字符串或 `@文件路径`
- 协议：HTTP/1.1（默认）、HTTP/2
- HTTPS：默认校验证书；支持 `--insecure` 跳过校验、`--cacert` 指定根证书
- 连接调优：请求超时（`--timeout`）、失败重试（`--retries`）、连接保活（`--keepalive`）均可配
- 精准 QPS 限流：令牌桶（零启动突发，实测误差 <1%）
- 自定义请求头：`-H "Key: Value"` 可重复传入；数据集记录可携带每请求请求头
- 数据集驱动：JSON 数据集每次请求随机取一条，覆盖 method/URL/headers/body
- 统计：HdrHistogram 紊秒精度延迟分布（min/mean/max/p50/p90/p95/p99/p99.9）、QPS、错误率、状态码分布、总字节数、错误 TopN、原始样本缓冲（上限 20 万条，每条含 bytes）
- 导出：console 控制台 / JSON（含原始样本）/ CSV 汇总
- CI 集成：`--p99-threshold <ms>` 设定性能阈值，p99 超阈值时进程以退出码 2 退出，便于流水线自动判断
- 基准测试：criterion.rs 微基准覆盖统计引擎（record/summarize）与令牌桶（acquire），支持 `cargo bench --no-run` 验证
- 优雅停止：Ctrl+C 排空在途请求后退出；实时进度条（indicatif）

---

## 环境要求

| 项 | 要求 |
|----|------|
| OS | Windows / Linux / macOS |
| Rust | stable ≥ 1.70（本仓库在 stable 1.98 验证） |
| 工具链 | **GNU 工具链**（Windows 本机无 VS Build Tools 时，MSVC 无法链接） |
| 网络 | 压测目标可达；公网目标需外网 |

> Windows 注意：若报缺失 `libgcc_s_seh-1.dll`，将 `C:\mingw64\bin` 加入 PATH。

---

## 快速开始

```powershell
# 1. 构建
cargo build

# 2. 运行测试（25 个单测：含 POST 报文、自定义请求头、YAML 合并优先级、状态码分布、纳秒分位数、Semaphore 并发限制、p99 阈值合并优先级）
cargo test

# 3. 编译基准测试（criterion：统计引擎 + 令牌桶）
cargo bench --no-run

# 4. 查看帮助
.\target\debug\loadtester.exe --help

# 5. 开始压测
.\target\debug\loadtester.exe https://example.com -n 100
```

---

## 命令行参数

| 参数 | 简写 | 默认 | 说明 |
|------|------|------|------|
| `<URL>` | — | 必填* | 压测目标，也可在配置文件 `url` 字段提供 |
| `--requests` | `-n` | — | 总请求数（与 `--duration` 二选一） |
| `--duration` | — | — | 压测时长（秒） |
| `--concurrency` | `-C` | 1 | 并发 worker 数 |
| `--method` | `-X` | GET | HTTP 方法（支持自定义） |
| `--data` | `-d` | — | 请求体：字面字符串或 `@文件路径` |
| `--header` | `-H` | — | 自定义请求头 `Key: Value`，可重复传入多个 |
| `--protocol` | — | http/1.1 | `http/1.1` \| `http/2` |
| `--timeout` | — | 30 | 单请求超时（秒） |
| `--retries` | — | 0 | 网络失败/超时的重试次数 |
| `--keepalive` | — | 30 | TCP/空闲连接保活（秒） |
| `--qps` | — | 不限流 | QPS 限流（令牌桶） |
| `--dataset` | — | — | 数据集 JSON 文件（每条记录可覆盖 method/URL/headers/body） |
| `--insecure` | — | false | 跳过 TLS 证书校验 |
| `--cacert` | — | — | 自定义根证书 PEM 路径 |
| `--format` | — | console | `console` \| `json` \| `csv` |
| `--output` | `-o` | — | 导出文件路径（json/csv 时建议指定） |
| `--config` | `-c` | — | YAML 配置文件（CLI 显式参数优先） |
| `--p99-threshold` | — | — | p99 延迟阈值（毫秒），超阈值时进程退出码为 2（CI 信号） |

\* 若 `-n` 与 `--duration` 均未指定，默认打满 10,000 请求。

---

## 使用示例

```powershell
# 基础压测：100 请求
.\target\debug\loadtester.exe https://example.com -n 100

# 高并发 + 限流：50 并发、200 QPS、持续 5 秒
.\target\debug\loadtester.exe http://127.0.0.1:8080/ -C 50 --duration 5 --qps 200

# POST 请求体（字面量 / 文件）
.\target\debug\loadtester.exe http://127.0.0.1:8080/submit -n 20 -X POST -d "{\"k\":1}"
.\target\debug\loadtester.exe http://127.0.0.1:8080/submit -n 20 -X POST -d "@body.json"

# 自定义请求头（需要鉴权/内容类型的接口；-H 可重复）
.\target\debug\loadtester.exe http://127.0.0.1:8080/api -n 20 `
  -H "Authorization: Bearer token123" -H "Content-Type: application/json"

# HTTP/2
.\target\debug\loadtester.exe https://example.com -n 20 --protocol http/2

# HTTPS 自签名 / 自定义根证书
.\target\debug\loadtester.exe https://192.168.1.10:8443 -n 20 --insecure
.\target\debug\loadtester.exe https://intranet.example.com -n 20 --cacert ca.pem

# 数据集驱动（每次请求随机取一条记录，覆盖 method/url/headers/body）
.\target\debug\loadtester.exe http://127.0.0.1:8080/ -n 30 --dataset data.json

# 结果导出
.\target\debug\loadtester.exe https://example.com -n 100 --format json -o report.json
.\target\debug\loadtester.exe https://example.com -n 100 --format csv -o report.csv

# 超时 10 秒、失败重试 2 次、连接保活 60 秒
.\target\debug\loadtester.exe http://127.0.0.1:8080/ -n 100 --timeout 10 --retries 2 --keepalive 60

# CI 阈值：p99 > 500ms 时进程以退出码 2 退出（便于流水线判断）
.\target\debug\loadtester.exe https://example.com -n 200 --p99-threshold 500
if ($LASTEXITCODE -eq 2) { Write-Error "性能不达标" }

# 配置文件启动（未在命令行写出的参数才取配置文件中的值）
.\target\debug\loadtester.exe -c config.yaml.example

# 混用：主体参数走 YAML，仅临时把并发改成 16（CLI 覆盖 YAML 的 concurrency）
.\target\debug\loadtester.exe -c config.yaml.example -C 16
```

### 数据集格式（`--dataset`）

```json
[
  {
    "method": "POST",
    "url": "http://127.0.0.1:8080/api/1",
    "headers": {"X-Trace-Id": "random-1", "Content-Type": "application/json"},
    "body": "{\"user\":\"a\"}"
  },
  {"method": "DELETE", "url": "http://127.0.0.1:8080/api/2"},
  {"body": "{\"user\":\"b\"}"}
]
```

每条记录的 `method`、`url`、`headers`、`body` 均可选，缺省时回退到命令行全局值：
- 记录未写 `method`/`url`/`body` -> 用 `-X`/`<URL>`/`-d` 的值；
- 记录的 `headers` 与全局 `-H` 合并；同名请求头（大小写不敏感）以记录中的值为准。

---

## YAML 配置文件

参数较多时可用 `-c config.yaml` 从配置文件启动，完整字段见 [config.yaml.example](config.yaml.example)：

```yaml
url: "https://example.com"
method: "POST"
concurrency: 8
requests: 10000                 # 或 duration: 30
protocol: "http/1.1"            # http/1.1 | http/2
qps: 1000
data: "@body.json"              # 也可直接写字面字符串；字段名也支持 body
headers:
  - "Authorization: Bearer token123"
  - "Content-Type: application/json"
timeout_secs: 30                # 单请求超时
max_retries: 2                  # 失败重试次数
keepalive_secs: 30              # 连接保活
insecure: false                 # true 跳过 TLS 证书校验
cacert: "./ca.pem"              # 自定义根证书
dataset: "./dataset_mixed.json"
format: "json"                  # console | json | csv
output: "./report.json"
p99_threshold_ms: 500           # 可选：p99 超阈值（毫秒）时退出码 2（CI 信号）
```

**合并优先级**：命令行显式参数 > YAML 配置文件 > 程序内置默认值。例如 YAML 写了 `concurrency: 8`，命令行再带 `-C 20`，最终生效 20；命令行没传的参数才回退到 YAML。请求头按"YAML 在前、CLI 在后"合并，同名头（大小写不敏感）以 CLI 为准。

---

## 输出解读

控制台示例：

```
----- 压测汇总 -----
总请求数   : 500
QPS        : 860.6
错误数     : 0 (0.00%)
延迟 (ms)  : min=3 mean=46.9 max=453 p50=15 p90=42 p95=420 p99=444 p99.9=452
总字节数   : 1.85 MB (1939812 bytes)
状态码分布 : 200=500
错误 TopN   :
  x12  连接超时: ...
```

- **QPS**：每秒完成请求数
- **错误数/率**：4xx/5xx + 网络错误（口径后续可配置为仅 5xx）
- **延迟（纳秒精度）**：内部以纳秒记录并参与分位数计算，输出时换算为毫秒展示；覆盖 p50 / p90 / p95 / p99 / p99.9 五档分位数
- **总字节数**：所有请求接收到的响应体字节数累加（HTTP 层 body 大小，不含响应头）
- **状态码分布**：按状态码（200/404/500/...，网络错误归入 0）计数

JSON 结构：`summary`（含 `total`、`errors`、`qps`、`error_rate`、`total_bytes`、`latency_ms{min/mean/max/p50/p90/p95/p99/p99.9}`、`status_code_distribution`）+ `top_errors` + `samples`（每条含 `status_code`、`elapsed_ms`、`bytes`、`http_version`）。

CSV 表头：`total,errors,qps,error_rate,min_ms,mean_ms,max_ms,p50_ms,p90_ms,p95_ms,p99_ms,p999_ms,total_bytes`。

---

## 项目结构

```
rust/
├── Cargo.toml            # 依赖与构建配置
├── config.yaml.example   # 配置文件样例
├── src/
│   ├── main.rs           # 入口 / 控制编排 / CI 阈值退出码（A）
│   ├── lib.rs            # lib target：让 bench 与 bin 共享模块
│   ├── cli.rs            # 命令行解析（D）
│   ├── config.rs         # YAML + CLI 合并（D）
│   ├── http_client.rs    # Reqwest 客户端封装：TLS/池/重试/协议（B）
│   ├── worker.rs         # dispatcher + Semaphore + JoinSet + 令牌桶（A）
│   ├── stats.rs          # HdrHistogram + 原子计数 + 错误 TopN（C）
│   ├── reporter.rs       # console/json/csv 输出（C）
│   └── dataset.rs        # 数据集随机化（D）
└── benches/
    ├── stats_bench.rs    # criterion 微基准：record 与 summarize（100k 样本）
    └── worker_bench.rs   # criterion 微基准：TokenBucket::acquire（多档 QPS）
```

分层：命令行入口（cli）→ 配置（config）→ 控制编排（main）→ dispatcher + Semaphore（worker）→ HTTP 客户端（http_client）→ 统计聚合（stats）→ 输出导出（reporter）。

---

## 团队分工（4 人）

| 角色 | 模块 | 说明 |
|------|------|------|
| A（组长） | `worker.rs`、`main.rs` | 并发编排、令牌桶接入、整体集成 |
| B | `http_client.rs` | 协议/TLS/连接池/重试 |
| C | `stats.rs`、`reporter.rs` | 统计引擎与输出导出 |
| D | `cli.rs`、`config.rs`、`dataset.rs` | 参数/配置合并、限流与数据集 |

## 开发路线（已完成状态）

- [x] 第 1 周：CLI + 配置合并 + 单请求链路
- [x] 第 2 周：Worker Pool + 时长模式 + 优雅停止 + 进度条
- [x] 第 3 周：统计引擎（min/mean/max、p50/p90/p95/p99/p99.9 纳秒精度、状态码分布、总字节数、错误 TopN、样本缓冲）
- [x] 第 4 周：精准令牌桶 + 数据集随机化
- [x] 第 5 周：HTTP/2 + JSON/CSV 完整导出
- [x] 第 6 周：测试（25/25 通过）、冒烟验证
- [x] 增强 1：并发模型由"N 个常驻 worker loop"重构为 dispatcher + `tokio::sync::Semaphore` + `JoinSet`，严格限制同时在途请求数
- [x] 增强 2：criterion.rs 微基准覆盖统计引擎与令牌桶（`cargo bench --no-run` 已验证）
- [x] 增强 3：`--p99-threshold <ms>` 性能阈值 + 退出码 2（CI 流水线信号）

### 已知 TODO / 后续增强

- HTTP/2 由 prior-knowledge 改为 ALPN 协商（目标不支持 h2 时更健壮）
- `4xx/5xx` 错误口径可配置（如仅 5xx 计为错误）
- 分布式压测 / 优雅停止信号细化（如自定义超时）
- 4xx/5xx 口径可配置后，再扩展 `--error-rate-threshold` 等多维度 CI 阈值

---

## 常见问题

1. **PowerShell 显示 `NativeCommandError`**：程序输出 stderr 时 PowerShell 的误报，非程序错误，加 `2>$null` 即可。
2. **构建告警 `corrupt .drectve`**：mingw 链接 MSVC 导入库的无害噪音，可忽略。
3. **HTTP/2 失败**：目标服务必须支持 h2（当前为 prior-knowledge，不支持会直接失败）。
4. **请仅压测你有权压的目标**，遵守目标服务的限流与合规要求。
