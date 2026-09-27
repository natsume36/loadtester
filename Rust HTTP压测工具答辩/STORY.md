# STORY.md — Rust HTTP 压测工具 · 项目答辩

## ① 用户意图对齐

**目标受众**：实训项目答辩评审组 —— 校内指导老师、专业教师，可能含企业导师。他们关心"是不是真做出来了""技术选型有没有依据""性能结论站不站得住"，而不是营销故事。

**核心目标**：讲完后让评审相信三件事 ——（1）这是一个从零实现的、功能完整的专业级 HTTP 压测工具，不是调库拼装；（2）每一个关键设计（并发模型、限流器、统计引擎）都有明确的取舍理由；（3）性能结论由 criterion 基准与 25 个单测共同支撑，可复现、可追溯。

**PPT 长度**：18 页（对应 12–15 分钟答辩）。Hero 页配额 4 页（22%）。

**视觉调性**：学术严谨 · 蓝白克制 · 证据驱动 · 数据密集 · 无营销感。

**内容边界**：
- 必讲：选题价值、技术选型依据、分层架构、并发模型重构、令牌桶精度、HdrHistogram 统计引擎、criterion 基准数据、测试验证与 CI 阈值、团队分工与展望。
- 不讲：环境安装步骤、逐行代码讲解、库的 API 用法。
- 禁碰：杜撰任何数据。全篇出现的数值仅来自 README、设计文档与用户提供的 criterion 实测结果（`stats_record_one_sample` 49.33 ns、`stats_summarize_100k_samples` 16.27 µs、令牌桶四档 QPS 实测中位数）。

---

## ② 页面布局骨架

**页面总数与分章**：共 18 页，分 4 章。

- 第 1 章「选题背景与技术选型」：P3 扉页、P4 背景与价值定位、P5 技术选型决策
- 第 2 章「系统架构与核心实现」：P6 扉页、P7 分层架构、P8 并发模型、P9 令牌桶限流器、P10 统计引擎、P11 工程化能力
- 第 3 章「基准测试与验证」：P12 扉页、P13 统计引擎基准、P14 令牌桶四档精度、P15 测试验证与 CI 阈值
- 第 4 章「总结与展望」：P16 扉页、P17 成果总结与团队分工、P18 感谢页

**目录↔章节扉页契约**：目录声明 4 个章节，全篇恰好 4 个 `type: section` 扉页，编号 01–04 连续，标题与页码区间逐字对应：

| 目录条目 | 编号 | 标题 | 页码区间 | 对应扉页 |
| :-- | :-- | :-- | :-- | :-- |
| 一 | 01 | 选题背景与技术选型 | P3–P5 | P3 |
| 二 | 02 | 系统架构与核心实现 | P6–P11 | P6 |
| 三 | 03 | 基准测试与验证 | P12–P15 | P12 |
| 四 | 04 | 总结与展望 | P16–P18 | P16 |

**Hero 页定位**：P1 封面（默认 Hero）、P8 并发模型（技术高潮）、P13 统计引擎基准（数据高潮）、P18 感谢页（默认 Hero）。共 4 页 = 22%，落在 20–30% 区间；相邻 Hero 间隔 ≥4 页。

**rhythm 曲线**：
P1 peak / P2 transition / P3 transition / P4 valley / P5 valley / P6 transition / P7 valley / P8 peak / P9 valley / P10 valley / P11 transition / P12 transition / P13 peak / P14 valley / P15 valley / P16 transition / P17 valley / P18 peak。
无任何连续 ≥3 页 valley。

**非对称版式预算**：18 页中 10 页使用非对称版式（P2、P4、P5、P7、P8、P9、P10、P11、P13、P15）= 56% ≥ 40%。

**对称版式预算**：仅 P14「图表+洞察」、P17「上下分栏」两页为对称版式（封面/扉页/感谢页属仪式页，不计入内容页预算）。

**版式多样性约束**：`N卡片横排` 全篇 0 次使用；`左大图+右侧文字` 与 `非对称双栏` 合计 4 页（P4、P9、P10、P15）= 22% ≤ 40%；相邻页版式无一重复。

**L1 视觉统一**：全篇 L1 主视觉统一为「流程 / 结构 SVG」（架构图、并发时序图、算法流程图、数据条形图），单类占比 100% ≥ 60%。

---

## ③ 页面大纲

### P1 封面

| 字段 | 内容 |
| :-- | :-- |
| title | Rust HTTP 压测工具 —— 高并发性能测试工具的设计与实现 |
| type | cover |
| role | hero |
| rhythm | peak |
| layout | 全屏视觉+大标题 |
| visual | L1: 深蓝几何构图 + 令牌/波形线稿（SVG，背景铺满） |
| visual_role | atmosphere |
| density | 字数约 60 / 图片 0 张 / 留白约 30% |
| anti_pattern | 禁止党政红金、印章、烟花、金装饰、人物大头照；禁止纯色矩形 + 单行文字的平铺式封面 |
| description | 中英文双标题 + 单位 + 汇报人 + 时间。英文长标题 "Design and Implementation of a High-Concurrency HTTP Load Testing Tool in Rust" 建立学术质感 |

### P2 目录

| 字段 | 内容 |
| :-- | :-- |
| title | 目录 |
| type | catalog |
| role | supporting |
| rhythm | transition |
| layout | 左标题+右内容（左 30% 深蓝标题栏 + 右 70% 四条目录项） |
| visual | L3: 章节编号方块 |
| visual_role | evidence |
| density | 字数约 120 / 图片 0 张 / 留白约 25% |
| anti_pattern | 禁止圆球编号、书法繁体数字；禁止英文比中文更抢眼；禁止 6 条以上目录项 |
| description | 4 个条目，编号 01–04 与章节扉页逐字一致。右侧每条含编号、标题、页码区间。四章覆盖全部 18 页 |

### P3 章节扉页 01

| 字段 | 内容 |
| :-- | :-- |
| title | 选题背景与技术选型 |
| type | section |
| role | transition |
| rhythm | transition |
| layout | 全屏视觉+大标题（左侧 240px 巨型数字 01 + 右侧标题与要点） |
| visual | L3: 浅蓝章节编号 |
| visual_role | atmosphere |
| density | 字数约 90 / 图片 0 张 / 留白约 35% |
| anti_pattern | 禁止四卡片预览；禁止铺满正文段落；禁止"PART 02"字样 |
| description | 大数字 01 + 本章标题 + 2 个要点（为什么自研、选型凭什么）。承接目录第 1 条，逐字一致 |

### P4 背景与价值定位

| 字段 | 内容 |
| :-- | :-- |
| title | 为什么要自己造一把尺子 |
| type | content |
| role | supporting |
| rhythm | valley |
| layout | 非对称双栏（65:35） |
| visual | L1: 工具链断裂示意图（左 65%，SVG 双轨流程） |
| visual_role | evidence |
| density | 字数约 260 / 图片 1 张 / 留白约 20% |
| anti_pattern | 禁止等分 50:50 双栏；禁止大留白；禁止把结论塞进装饰图 |
| description | 调试用 curl/Postman、压测用 wrk/ab/JMeter —— 两套工具、两套口径、上下文切换成本高。自研工具的价值是把请求发生、限流、统计、导出收敛到一条链路，指标口径唯一。左栏画"两套工具 vs 一条链路"对照流程 |

### P5 技术选型决策

| 字段 | 内容 |
| :-- | :-- |
| title | 八个依赖，八次取舍 |
| type | content |
| role | supporting |
| rhythm | valley |
| layout | 左标题+右内容（左 32% 选型原则，右 68% 依赖矩阵表） |
| visual | L3: 表格行内标签 |
| visual_role | evidence |
| density | 字数约 250 / 图片 0 张 / 留白约 18% |
| anti_pattern | 禁止把依赖罗列成无理由的图标墙；禁止用营销词包装选型 |
| description | Tokio full / reqwest 0.12 / rustls / HdrHistogram 7.5 / clap 4.5 / serde / indicatif / criterion。核心取舍两条：reqwest 关默认特性换 rustls 摆脱 OpenSSL 原生依赖；release 开 LTO + opt-level 3，dev 用 opt-level 1 平衡编译速度 |

### P6 章节扉页 02

| 字段 | 内容 |
| :-- | :-- |
| title | 系统架构与核心实现 |
| type | section |
| role | transition |
| rhythm | transition |
| layout | 全屏视觉+大标题（左侧 240px 巨型数字 02 + 右侧标题与要点） |
| visual | L3: 浅蓝章节编号 |
| visual_role | atmosphere |
| density | 字数约 90 / 图片 0 张 / 留白约 35% |
| anti_pattern | 禁止四卡片预览；禁止与封面雷同；禁止满版蓝底 |
| description | 大数字 02 + 标题 + 2 个要点（分层解耦、RAII 管并发）。与 P3 版式同构但配色加深一档，全篇章节扉页样式统一 |

### P7 系统分层架构

| 字段 | 内容 |
| :-- | :-- |
| title | 七层职责，一条单向数据流 |
| type | content |
| role | supporting |
| rhythm | valley |
| layout | 上大图+下方卡片（上 58% 架构图，下 42% 三张职责卡） |
| visual | L1: 分层架构图（SVG，上区 58%，cli → config → main → worker → http_client → stats → reporter 七层串联 + 数据集/限流旁路） |
| visual_role | anchor |
| density | 字数约 240 / 图片 1 张 / 留白约 15% |
| anti_pattern | 禁止单栏线性列表；禁止用图标代替层级关系；禁止把架构图缩成 200×70 角标 |
| description | 命令行入口 → 配置合并 → 控制编排 → 并发调度 → HTTP 客户端 → 统计聚合 → 输出导出。上层只依赖下层的公开接口，`lib.rs` 让 bench 与 bin 共享同一份实现——这是后续能做微基准的前提 |

### P8 并发模型

| 字段 | 内容 |
| :-- | :-- |
| title | 从 N 个常驻循环到 Semaphore + JoinSet |
| type | content |
| role | hero |
| rhythm | peak |
| layout | 全幅图+骑线文字（全幅并发时序 SVG 铺底，左侧 38% 骑线文字卡） |
| visual | L1: 并发时序图（SVG，铺满全幅：dispatcher 派发 → acquire_owned 取 permit → task 执行 → RAII 归还） |
| visual_role | anchor |
| density | 字数约 230 / 图片 1 张 / 留白约 20% |
| anti_pattern | 禁止等宽卡片横排；禁止把并发现场上限塞进图表卡角落；禁止 L3 角标顶替 L1 |
| description | 重构前是"N 个常驻 worker loop"，在途请求数不可控；重构后 dispatcher 每派发一个请求前先 `acquire_owned()` 一个 permit，permit 由 spawned task 持有、结束时 RAII 归还，`JoinSet` 统一排空。单测 `spawn_bounded_caps_inflight_within_permits` 断言峰值 ≤ permits 且至少达到 permits —— 既不超限，也不浪费槽位 |

### P9 令牌桶限流器

| 字段 | 内容 |
| :-- | :-- |
| title | 零启动突发的令牌桶 |
| type | content |
| role | supporting |
| rhythm | valley |
| layout | 左大图+右侧文字（左 58% 算法流程 SVG，右 42% 要点卡） |
| visual | L1: 令牌桶状态机（SVG：refill 按 elapsed × rate 补币 → tokens ≥ 1 扣币放行 → 否则等 (1−tokens)/rate + 1ms） |
| visual_role | anchor |
| density | 字数约 240 / 图片 1 张 / 留白约 18% |
| anti_pattern | 禁止 50:50 等分双栏；禁止把流程图替换为纯文字步骤列表；禁止省略"临界区外等待"这一关键设计 |
| description | 初始令牌为 0 —— 这是刻意的：若初始给满 rate 个令牌，启动瞬间会打出一次 rate 量级的突发，平均速率失真。容量 = 速率，允许空闲后补回最多 1 秒的量。锁内只做计算，`sleep` 放在临界区外，避免 async 任务持锁过夜 |

### P10 统计引擎

| 字段 | 内容 |
| :-- | :-- |
| title | 纳秒级延迟分布怎么算出来的 |
| type | content |
| role | supporting |
| rhythm | valley |
| layout | 非对称双栏（60:40） |
| visual | L1: HdrHistogram 分桶原理 SVG（左 60%）+ 直方图阶梯示意 |
| visual_role | evidence |
| density | 字数约 250 / 图片 1 张 / 留白约 18% |
| anti_pattern | 禁止用"平均值"包装延迟统计；禁止把 p50/p99 当作并列指标罗列而不解释尾部 |
| description | 传统做法用平均值聚合，均值会掩盖长尾，而压测最该看的就是尾部。改用 HdrHistogram（significance = 3），记录时容忍固定相对误差、内存可控，能直接取 p50/p90/p95/p99/p99.9。总量/错误数/字节数走 `AtomicU64` 无锁累加；状态码分布与样本缓冲（上限 20 万条）走 `Mutex<BTreeMap>` —— 热路径无锁，冷路径才加锁 |

### P11 工程化能力

| 字段 | 内容 |
| :-- | :-- |
| title | 不止能压，还要能进流水线 |
| type | content |
| role | supporting |
| rhythm | transition |
| layout | 左标题+右内容（左 30% 能力轴，右 70% 四项能力卡 + CLI 示例） |
| visual | L3: 能力标签 + CodeBlock |
| visual_role | evidence |
| density | 字数约 260 / 图片 0 张 / 留白约 18% |
| anti_pattern | 禁止堆砌参数清单而不讲优先级规则；禁止把 CI 阈值讲成附加功能 |
| description | 配置三层合并：CLI 显式参数 > YAML > 内置默认值；请求头按"YAML 在前、CLI 在后"合并，同名头 CLI 覆盖。数据集驱动：JSON 每次请求随机取一条，覆盖 method/url/headers/body，缺省回退全局值。导出 console / JSON / CSV 三态。`--p99-threshold <ms>` 超阈值以退出码 2 退出 —— 把性能门槛变成流水线能自动判断的信号 |

### P12 章节扉页 03

| 字段 | 内容 |
| :-- | :-- |
| title | 基准测试与验证 |
| type | section |
| role | transition |
| rhythm | transition |
| layout | 全屏视觉+大标题（左侧 240px 巨型数字 03 + 右侧标题与要点） |
| visual | L3: 浅蓝章节编号 |
| visual_role | atmosphere |
| density | 字数约 90 / 图片 0 张 / 留白约 35% |
| anti_pattern | 禁止四卡片预览；禁止提前泄露具体数值；禁止铺满正文 |
| description | 大数字 03 + 标题 + 2 个要点（微基准怎么测、结论怎么读）。样式与 P3、P6 严格同构 |

### P13 统计引擎基准

| 字段 | 内容 |
| :-- | :-- |
| title | 记录一条样本 49.33 纳秒 |
| type | content |
| role | hero |
| rhythm | peak |
| layout | 巨型数字+洞察（左侧 ≥120px 巨型数字群 + 右侧解读） |
| visual | L1: 巨型数字 49.33 / 16.27 + 对比条形 SVG |
| visual_role | anchor |
| density | 字数约 210 / 图片 1 张 / 留白约 22% |
| anti_pattern | 禁止把核心数字塞进等宽卡片；禁止只摆数字不给判断；禁止把 10 万样本聚合说成"很快"而不给量级 |
| description | `stats_record_one_sample` 中位数 49.33 ns —— HdrHistogram 写入 + atomic 计数 + push_sample 三件事打包完成，标称 3–6 ns 的直方图写入在完整链路下仍守住百纳秒内量级。`stats_summarize_100k_samples` 16.27 µs —— 10 万样本取 5 档分位数 + 克隆状态码分布只要 16 微秒，意味着统计开销不会污染被测对象的延迟读数 |

### P14 令牌桶四档精度

| 字段 | 内容 |
| :-- | :-- |
| title | 限流精度：理论周期 vs 实测中位数 |
| type | content |
| role | supporting |
| rhythm | valley |
| layout | 图表+洞察（上 62% 分组柱状图，下 38% 洞察条 + 关键数字） |
| visual | Chart(分组柱状图：四档 QPS 的理论周期与实测中位数) + L1: 上方引导问题条 |
| visual_role | evidence |
| density | 字数约 200 / 图片 1 张 / 留白约 18% |
| anti_pattern | 禁止只给表格不给洞察；禁止把偏差说成"误差极小"而不量化；禁止省略 1ms 余量这一设计来源 |
| description | 四档实测中位数 12.300 ms / 1.119 ms / 111.8 µs / 11.19 µs。实测 = 理论周期 + 固定开销，固定开销来自 Mutex 临界区约 1.1 µs、`tokio::time::sleep` 精度损失、以及 1 ms 余量补偿。QPS ≥ 1000 时偏差占比 < 12%，工程可用区间内精度达标；100 QPS 档 +2.30 ms 主要由 1 ms 余量与调度抖动构成 |

### P15 测试验证与 CI 阈值

| 字段 | 内容 |
| :-- | :-- |
| title | 25 个单测 + 一条退出码规则 |
| type | content |
| role | supporting |
| rhythm | valley |
| layout | 非对称双栏（62:38） |
| visual | L1: 验证链路 SVG（左 62%）+ 右栏结论卡 |
| visual_role | evidence |
| density | 字数约 250 / 图片 1 张 / 留白约 18% |
| anti_pattern | 禁止只报"测试通过"不讲覆盖点；禁止等分双栏；禁止把退出码机制描述为可选装饰 |
| description | 25/25 单测覆盖 POST 报文送达、自定义请求头、YAML 合并优先级（含 p99 阈值三层）、状态码分布、纳秒分位数单调性、Semaphore 并发上限、空统计兜底。基准侧两个 bench 均可用 `cargo bench --no-run` 编译验证。`--p99-threshold` 超限退出码 2，把"性能是否达标"交给流水线自动判定 |

### P16 章节扉页 04

| 字段 | 内容 |
| :-- | :-- |
| title | 总结与展望 |
| type | section |
| role | transition |
| rhythm | transition |
| layout | 全屏视觉+大标题（左侧 240px 巨型数字 04 + 右侧标题与要点） |
| visual | L3: 浅蓝章节编号 |
| visual_role | atmosphere |
| density | 字数约 90 / 图片 0 张 / 留白约 35% |
| anti_pattern | 禁止四卡片预览；禁止在此页罗列成果数据；禁止铺满正文 |
| description | 大数字 04 + 标题 + 2 个要点（交付了什么、下一步往哪走）。第四章扉页，与前三章同构 |

### P17 成果总结与团队分工

| 字段 | 内容 |
| :-- | :-- |
| title | 六周交付与四人分工 |
| type | content |
| role | supporting |
| rhythm | valley |
| layout | 上下分栏（上 55% 六周路线时间轴，下 45% 四人分工卡） |
| visual | L1: 六周路线时间轴 SVG（上 55%）+ L3: 角色编号 |
| visual_role | evidence |
| density | 字数约 240 / 图片 1 张 / 留白约 18% |
| anti_pattern | 禁止把分工写成无信息量的"负责模块 X"；禁止等宽四卡横排代替时间轴 |
| description | 六周路线全部打勾：CLI+配置 → Worker Pool → 统计引擎 → 限流+数据集 → HTTP/2+导出 → 测试与冒烟，另含三项增强（并发模型重构、criterion 微基准、CI 阈值）。分工按模块边界切：A 并发编排与集成、B 协议/TLS/连接池/重试、C 统计与输出导出、D CLI/配置合并/数据集。后续增强方向：HTTP/2 改 ALPN 协商、错误口径可配置并扩展多维度 CI 阈值、分布式压测 |

### P18 感谢页

| 字段 | 内容 |
| :-- | :-- |
| title | 感谢聆听，恳请批评指正 |
| type | ending |
| role | hero |
| rhythm | peak |
| layout | 居中金句/巨型数字 |
| visual | L1: 与封面同源的深蓝几何构图（复用封面包边语言，不复制版式） |
| visual_role | atmosphere |
| density | 字数约 40 / 图片 0 张 / 留白约 35% |
| anti_pattern | 禁止烟花、金装饰、印章；禁止新增未在前面出现过的信息 |
| description | 延续封面顶条体系收尾，一行主句 + 一行答辩信息。保持克制，不做情绪化表达 |

---

## Checklist 自检

- Hero 页 4/18 = 22%，区间内；`N卡片横排` 0 次；非对称版式 56% ≥ 40%；无相邻版式重复 ✓
- `左大图+右侧文字` + `非对称双栏` 共 4 页 = 22% ≤ 40% ✓
- 每页均含 role / rhythm / visual_role / anti_pattern ✓
- 含关键数据的页（P13 / P14 / P15）均写成"数据 + 判断"两段式 ✓
- 目录声明 4 章，`type: section` 扉页恰好 4 个，编号 01–04 连续，标题与页码区间与目录逐字一致 ✓
- 连续 valley 最长 2 页，未触发 ≥3 页规则 ✓
