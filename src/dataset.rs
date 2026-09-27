//! 数据集驱动（D 角色交付）。
//! 读取 JSON 数据集（记录数组），每次请求随机取一条记录的 body/URL/headers，
//! 实现"数据集驱动随机化压测"。每条记录可覆盖请求的 body 与 URL。

use anyhow::Result;
use rand::Rng;
use serde::Deserialize;
use std::collections::BTreeMap;

/// 数据集中的一条记录；所有字段均可选，缺省时回退到命令行/全局配置。
#[derive(Debug, Clone, Deserialize)]
pub struct DatasetEntry {
    /// 本次请求的 body（可选，缺省则用统一 body）
    #[serde(default)]
    pub body: Option<String>,
    /// 覆盖请求 URL（可选）
    #[serde(default)]
    pub url: Option<String>,
    /// 覆盖 HTTP 方法（可选，如 GET/POST/PUT/DELETE）
    #[serde(default)]
    pub method: Option<String>,
    /// 附加请求头（可选；与全局请求头同名时覆盖全局值）
    #[serde(default)]
    pub headers: Option<BTreeMap<String, String>>,
}

/// 数据集：记录数组 + 随机抽取。
#[derive(Debug, Clone)]
pub struct Dataset {
    entries: Vec<DatasetEntry>,
}

impl Dataset {
    /// 从 JSON 文件加载数据集。
    pub fn load(path: &str) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("读取数据集 {path} 失败: {e}"))?;
        let entries: Vec<DatasetEntry> = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("解析数据集 {path} 失败: {e}"))?;
        anyhow::ensure!(!entries.is_empty(), "数据集 {path} 为空，无法用于随机化");
        Ok(Self { entries })
    }

    /// 随机取一条记录（均匀分布）。
    pub fn next_entry(&self) -> &DatasetEntry {
        let mut rng = rand::thread_rng();
        let idx = rng.gen_range(0..self.entries.len());
        &self.entries[idx]
    }

    #[allow(dead_code)] // 对外提供的常规集合方法
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)] // 对外提供的常规集合方法
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_supports_url_method_headers_body() {
        let json = r#"[{
            "method": "POST",
            "url": "http://127.0.0.1:8088/submit",
            "headers": {"X-Trace-Id": "abc", "Content-Type": "application/json"},
            "body": "{\"k\":1}"
        }]"#;
        let entries: Vec<DatasetEntry> = serde_json::from_str(json).unwrap();
        let e = &entries[0];
        assert_eq!(e.method.as_deref(), Some("POST"));
        assert_eq!(e.url.as_deref(), Some("http://127.0.0.1:8088/submit"));
        assert_eq!(e.body.as_deref(), Some("{\"k\":1}"));
        assert_eq!(e.headers.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn entry_all_fields_optional() {
        let entries: Vec<DatasetEntry> = serde_json::from_str("[{}]").unwrap();
        let e = &entries[0];
        assert!(e.method.is_none() && e.url.is_none() && e.body.is_none() && e.headers.is_none());
    }
}