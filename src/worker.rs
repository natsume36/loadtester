//! Worker Pool 层（A 角色交付）。
//!
//! 架构：dispatcher loop + `tokio::sync::Semaphore` + `tokio::task::JoinSet`。
//! - dispatcher 在主循环里按"请求数 / 时长"派发任务；
//! - 每派发一个请求前先 `acquire_owned()` 一个 Semaphore permit，
//!   permit 由 spawned task 持有并在结束时自动释放（RAII），
//!   从而把同时在途的请求数严格限制在 `config.concurrency` 之内；
//! - `JoinSet` 集中追踪所有在途任务，结束时 `join_next()` 排空。
//!
//! 保留：令牌桶 QPS 限流、Ctrl+C 优雅停止、indicatif 进度条、
//! 请求数/时长双模式、数据集每请求随机取条目、请求头合并规则。

use crate::config::Config;
use crate::dataset::Dataset;
use crate::http_client::HttpClient;
use crate::stats::Stats;
use anyhow::Result;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// 令牌桶限流器（QPS 精准限流）。
///
/// 用 `Instant` 记录最后补充时刻，按速率补充令牌；在临界区外等待，避免 async 中持锁。
#[derive(Clone)]
pub struct TokenBucket {
    inner: Arc<Mutex<TokenBucketInner>>,
}

struct TokenBucketInner {
    tokens: f64,
    capacity: u64,
    rate_per_sec: u64,
    last_fill: Instant,
}

impl TokenBucket {
    /// 按指定速率（个/秒）创建令牌桶，容量=速率（空闲一段时间后允许最多补回 1 秒的突发量）。
    pub fn new(rate: u64) -> Self {
        let rate = rate.max(1);
        Self {
            inner: Arc::new(Mutex::new(TokenBucketInner {
                // 初始令牌为 0：避免启动时一次性突发 rate 个请求，保证平均速率贴近设定值
                tokens: 0.0,
                capacity: rate,
                rate_per_sec: rate,
                last_fill: Instant::now(),
            })),
        }
    }

    /// 阻塞直到允许放行一个请求。
    pub async fn acquire(&self) {
        loop {
            let wait = {
                let mut inner = self.inner.lock().unwrap();
                inner.refill();
                if inner.tokens >= 1.0 {
                    inner.tokens -= 1.0;
                    None
                } else {
                    // 还需等待补齐 1 个令牌的时间；多给 1ms 余量保证不触发边界
                    Some(Duration::from_secs_f64(
                        (1.0 - inner.tokens) / inner.rate_per_sec as f64,
                    ) + Duration::from_millis(1))
                }
            };
            match wait {
                None => return,
                Some(d) => tokio::time::sleep(d).await,
            }
        }
    }
}

impl TokenBucketInner {
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_fill).as_secs_f64();
        self.last_fill = now;
        let add = elapsed * self.rate_per_sec as f64;
        self.tokens = (self.tokens + add).min(self.capacity as f64);
    }
}

/// 在 `JoinSet` 上派发一个被 `Semaphore` 限制并发的新任务。
///
/// 先 `acquire_owned()` 取得 permit（阻塞到有空闲槽位），
/// 然后 spawn 一个 async 任务，permit 由任务持有并在其结束时自动 drop 归还。
/// 因此 `JoinSet` 中同时未完成的任务数不会超过 `Semaphore` 的 permits 数。
///
/// 暴露为 pub(crate) 便于单元测试覆盖其并发约束。
pub(crate) async fn spawn_bounded<Fut>(
    set: &mut JoinSet<()>,
    sem: Arc<Semaphore>,
    fut: Fut,
) where
    Fut: Future<Output = ()> + Send + 'static,
{
    let permit = sem.acquire_owned().await.expect("Semaphore 关闭");
    set.spawn(async move {
        let _permit = permit; // RAII：task 结束时自动归还 permit
        fut.await;
    });
}

/// 顶层编排：dispatcher 按"请求数 / 时长"派发任务，`Semaphore` 限制同时在途请求数。
///
/// - `config.requests` 有值：打满该数量后停止；
/// - `config.duration` 有值：截止时间到即停止；
/// - 两者都没有：退化为默认请求数（`effective_total`）。
/// - Ctrl+C：置停止标记，dispatcher 在边界感知并结束，在途请求排空后返回。
pub async fn run(
    config: &Config,
    client: Arc<HttpClient>,
    stats: Arc<Stats>,
    dataset: Option<Arc<Dataset>>,
) -> Result<u64> {
    let workers = config.concurrency.max(1);
    let send_count = Arc::new(AtomicU64::new(0));
    let started = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));

    // 结束条件归一化：请求数模式优先；无任何条件时用默认请求数兜底。
    let total = config.requests.or_else(|| {
        if config.duration.is_none() {
            Some(config.effective_total())
        } else {
            None
        }
    });
    let deadline = config.duration.map(|secs| Instant::now() + Duration::from_secs(secs));

    let bucket = config.qps.map(TokenBucket::new);

    // 并发槽位：每个 permit = 一个在途请求
    let semaphore = Arc::new(Semaphore::new(workers));

    // Ctrl+C 优雅停止
    let stop_flag = stop.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        stop_flag.store(true, Ordering::Relaxed);
    });

    // 进度条
    let pb = indicatif::ProgressBar::new(total.unwrap_or(0));
    pb.set_style(
        indicatif::ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} {per_sec}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );

    let base_method = config.method.clone();
    let base_url = config.url.clone();
    let base_body = config.body.clone();
    let base_headers = config.headers.clone();

    let mut set: JoinSet<()> = JoinSet::new();

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        // 结束条件判定（请求数 / 截止时间）
        match total {
            Some(t) => {
                if started.fetch_add(1, Ordering::Relaxed) >= t {
                    break;
                }
            }
            None => {
                if deadline.is_some_and(|dl| Instant::now() >= dl) {
                    break;
                }
                started.fetch_add(1, Ordering::Relaxed);
            }
        }

        // 限流：令牌桶按 QPS 放行（与原 worker 行为一致）
        if let Some(b) = &bucket {
            b.acquire().await;
        }

        // 每请求参数：有数据集时随机取一条记录，其 method/url/body 缺省回退到全局值
        let entry = dataset.as_ref().map(|d| d.next_entry());
        let req_method = entry
            .as_ref()
            .and_then(|e| e.method.clone())
            .unwrap_or_else(|| base_method.clone());
        let req_url = entry
            .as_ref()
            .and_then(|e| e.url.clone())
            .unwrap_or_else(|| base_url.clone());
        let req_body = entry
            .as_ref()
            .and_then(|e| e.body.clone())
            .or_else(|| base_body.clone());

        // 请求头：全局头 + 数据集记录头；同名头（大小写不敏感）由数据集记录覆盖
        let mut req_headers = base_headers.clone();
        if let Some(entry_headers) = entry.as_ref().and_then(|e| e.headers.as_ref()) {
            for (name, value) in entry_headers {
                match req_headers
                    .iter_mut()
                    .find(|(global_name, _)| global_name.eq_ignore_ascii_case(name))
                {
                    Some(slot) => slot.1 = value.clone(),
                    None => req_headers.push((name.clone(), value.clone())),
                }
            }
        }

        let client = client.clone();
        let stats = stats.clone();
        let pb = pb.clone();
        let send_count = send_count.clone();

        // 在 Semaphore 限制下派发任务：permit 由 task 持有，drop 时归还
        spawn_bounded(&mut set, semaphore.clone(), async move {
            match client
                .send(&req_method, &req_url, &req_headers, req_body.as_deref())
                .await
            {
                Ok(resp) => stats.record(
                    resp.status_code,
                    resp.elapsed_ns,
                    resp.http_version,
                    resp.bytes_received,
                ),
                Err(e) => stats.record_error(&e.to_string()),
            }
            send_count.fetch_add(1, Ordering::Relaxed);
            pb.inc(1);
        })
        .await;
    }

    // 等待所有在途任务完成（permits 在此期间被逐个归还）
    while set.join_next().await.is_some() {}
    pb.finish_and_clear();

    Ok(send_count.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    /// 验证 `spawn_bounded` 把同时未完成任务数严格限制在 Semaphore permits 之内。
    ///
    /// 派发 200 个任务，每个任务 sleep 10ms 并在执行期间递增 in-flight、
    /// 递减时更新峰值；断言峰值 <= permits（=4）。
    /// 这等价于验证 `run()` 中"每派发一个请求前先 acquire permit"的并发约束。
    #[tokio::test(flavor = "current_thread")]
    async fn spawn_bounded_caps_inflight_within_permits() {
        let permits = 4usize;
        let sem = Arc::new(Semaphore::new(permits));
        let mut set: JoinSet<()> = JoinSet::new();

        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        for _ in 0..200 {
            let in_flight = in_flight.clone();
            let peak = peak.clone();
            spawn_bounded(&mut set, sem.clone(), async move {
                let cur = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                // 更新峰值（CAS）
                loop {
                    let p = peak.load(Ordering::SeqCst);
                    if cur <= p {
                        break;
                    }
                    if peak
                        .compare_exchange(p, cur, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
                in_flight.fetch_sub(1, Ordering::SeqCst);
            })
            .await;
        }

        // 排空所有任务
        while set.join_next().await.is_some() {}

        assert_eq!(
            in_flight.load(Ordering::SeqCst),
            0,
            "全部任务结束后 in-flight 应归零"
        );
        let peak_value = peak.load(Ordering::SeqCst);
        assert!(
            peak_value <= permits,
            "in-flight 峰值 {peak_value} 不应超过 Semaphore permits {permits}"
        );
        // 至少应该出现过同时有 permits 个任务在执行的情况
        assert!(
            peak_value >= permits,
            "峰值 {peak_value} 应至少达到 permits {permits}（证明并发槽位被充分利用）"
        );
    }
}
