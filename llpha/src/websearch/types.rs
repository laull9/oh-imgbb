use serde::{Deserialize, Serialize};
use serde_json::Value;

/// SearchEngineKind 表示当前支持的搜索引擎类型。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEngineKind {
    Aggregate,
    Bing,
    DuckDuckGo,
    SearxNg,
}

/// SearchResult 表示一条搜索结果。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// SearchResponse 表示一次搜索的结构化响应。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub engine: SearchEngineKind,
    pub query: String,
    pub results: Vec<SearchResult>,
    pub pages_fetched: usize,
}

impl SearchResponse {
    /// 将搜索结果转换为适合展示的纯文本。
    pub fn to_text(&self) -> String {
        self.results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                format!(
                    "{}. {}\n{}\n{}",
                    index + 1,
                    result.title,
                    result.url,
                    result.snippet
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// 将搜索结果转换为 JSON 值。
    pub fn to_json(&self) -> anyhow::Result<Value> {
        serde_json::to_value(self).map_err(Into::into)
    }

    /// 将搜索结果转换为 JSON 字符串。
    pub fn to_json_string(&self) -> anyhow::Result<String> {
        serde_json::to_string_pretty(self).map_err(Into::into)
    }
}

/// SearchPing 表示一次搜索引擎可用性探测结果。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchPing {
    pub engine: SearchEngineKind,
    pub available: bool,
    pub base_url: String,
    pub status: Option<u16>,
    pub latency_ms: u128,
    pub error: Option<String>,
    pub children: Vec<SearchPing>,
}

impl SearchPing {
    /// 创建单个搜索引擎 ping 结果。
    pub fn single(
        engine: SearchEngineKind,
        available: bool,
        base_url: String,
        status: Option<u16>,
        latency_ms: u128,
        error: Option<String>,
    ) -> Self {
        Self {
            engine,
            available,
            base_url,
            status,
            latency_ms,
            error,
            children: Vec::new(),
        }
    }

    /// 创建聚合搜索引擎 ping 结果。
    pub fn aggregate(children: Vec<SearchPing>) -> Self {
        let available = children.iter().any(|child| child.available);
        let latency_ms = children
            .iter()
            .map(|child| child.latency_ms)
            .max()
            .unwrap_or(0);
        let error = if available {
            None
        } else {
            Some(
                children
                    .iter()
                    .filter_map(|child| child.error.as_deref())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
            .filter(|value| !value.is_empty())
        };

        Self {
            engine: SearchEngineKind::Aggregate,
            available,
            base_url: children
                .iter()
                .find(|child| child.available)
                .or_else(|| children.first())
                .map(|child| child.base_url.clone())
                .unwrap_or_default(),
            status: None,
            latency_ms,
            error,
            children,
        }
    }
}

/// SearchPage 表示单页搜索解析结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchPage {
    pub results: Vec<SearchResult>,
    pub next_url: Option<String>,
}

impl SearchPage {
    /// 创建单页搜索解析结果。
    pub(crate) fn new(results: Vec<SearchResult>, next_url: Option<String>) -> Self {
        Self { results, next_url }
    }
}
