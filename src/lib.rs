//! Library target：让 bin 与外部 bench 共用同一组模块。
//!
//! criterion bench（benches/*.rs）通过 `loadtester::stats::Stats` /
//! `loadtester::worker::TokenBucket` 等路径访问内部类型，故需 lib target。

pub mod cli;
pub mod config;
pub mod dataset;
pub mod http_client;
pub mod reporter;
pub mod stats;
pub mod worker;
