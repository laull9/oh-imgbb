use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;

use crate::websearch::config::DEFAULT_SEARCH_LIMIT;
use crate::websearch::core::SearchEngine;
use crate::websearch::types::{SearchEngineKind, SearchPing, SearchResponse, SearchResult};

use super::{DuckDuckGoSearch, SearxNgSearch};

/// AggregateSearchBuilder 构建聚合搜索引擎。
#[derive(Clone, Default)]
pub struct AggregateSearchBuilder {
    engines: Vec<Arc<dyn SearchEngine>>,
    limit: Option<usize>,
}

impl AggregateSearchBuilder {
    /// 创建聚合搜索构建器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置聚合搜索最终返回条数。
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit.max(1));
        self
    }

    /// 添加一个子搜索引擎。
    pub fn engine<E>(mut self, engine: E) -> Self
    where
        E: SearchEngine + 'static,
    {
        self.engines.push(Arc::new(engine));
        self
    }

    /// 添加一个共享子搜索引擎。
    pub fn shared_engine(mut self, engine: Arc<dyn SearchEngine>) -> Self {
        self.engines.push(engine);
        self
    }

    /// 构建聚合搜索引擎。
    pub fn build(self) -> Result<AggregateSearch> {
        let limit = self.limit.unwrap_or(DEFAULT_SEARCH_LIMIT);
        let engines = if self.engines.is_empty() {
            vec![
                Arc::new(SearxNgSearch::builder().limit(limit).build()?) as Arc<dyn SearchEngine>,
                Arc::new(DuckDuckGoSearch::builder().limit(limit).build()?)
                    as Arc<dyn SearchEngine>,
            ]
        } else {
            self.engines
        };

        Ok(AggregateSearch { engines, limit })
    }
}

/// AggregateSearch 聚合多个搜索引擎并返回去重结果。
pub struct AggregateSearch {
    engines: Vec<Arc<dyn SearchEngine>>,
    limit: usize,
}

impl AggregateSearch {
    /// 创建默认聚合搜索引擎。
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// 创建聚合搜索构建器。
    pub fn builder() -> AggregateSearchBuilder {
        AggregateSearchBuilder::new()
    }

    /// 返回聚合搜索包含的子引擎数量。
    pub fn engine_count(&self) -> usize {
        self.engines.len()
    }

    /// 返回聚合搜索包含的子引擎类型。
    pub fn engine_kinds(&self) -> Vec<SearchEngineKind> {
        self.engines.iter().map(|engine| engine.kind()).collect()
    }
}

#[async_trait]
impl SearchEngine for AggregateSearch {
    /// 返回聚合搜索引擎类型。
    fn kind(&self) -> SearchEngineKind {
        SearchEngineKind::Aggregate
    }

    /// 执行聚合搜索并返回去重后的结构化结果。
    async fn search(&self, query: &str) -> Result<SearchResponse> {
        let mut pages_fetched = 0usize;
        let mut errors = Vec::new();
        let mut result_groups = Vec::new();

        for engine in &self.engines {
            match engine.search(query).await {
                Ok(response) => {
                    pages_fetched = pages_fetched.saturating_add(response.pages_fetched);
                    result_groups.push(response.results);
                }
                Err(err) => errors.push(format!("{:?}: {err}", engine.kind())),
            }
        }

        let results = merge_result_groups(result_groups, self.limit);
        if results.is_empty() && !errors.is_empty() {
            return Err(anyhow!("聚合搜索全部失败: {}", errors.join("; ")));
        }

        Ok(SearchResponse {
            engine: SearchEngineKind::Aggregate,
            query: query.to_string(),
            results,
            pages_fetched,
        })
    }

    /// 并发探测所有子搜索引擎并汇总可用状态。
    async fn ping(&self) -> Result<SearchPing> {
        let mut handles = Vec::with_capacity(self.engines.len());
        for engine in &self.engines {
            let engine = engine.clone();
            handles.push(tokio::spawn(async move {
                let kind = engine.kind();
                engine.ping().await.unwrap_or_else(|err| {
                    SearchPing::single(kind, false, String::new(), None, 0, Some(err.to_string()))
                })
            }));
        }

        let mut children = Vec::with_capacity(handles.len());
        for handle in handles {
            children.push(handle.await?);
        }

        Ok(SearchPing::aggregate(children))
    }
}

/// 轮询合并多组搜索结果并去重。
fn merge_result_groups(result_groups: Vec<Vec<SearchResult>>, limit: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();
    let max_group_len = result_groups.iter().map(Vec::len).max().unwrap_or_default();

    for index in 0..max_group_len {
        for group in &result_groups {
            let Some(result) = group.get(index) else {
                continue;
            };

            if seen_urls.insert(result.url.clone()) {
                results.push(result.clone());
            }

            if results.len() >= limit {
                return results;
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证默认聚合搜索包含内置引擎。
    #[test]
    fn aggregate_search_uses_default_engines() {
        let search = AggregateSearch::builder().limit(20).build().unwrap();

        assert_eq!(search.engine_count(), 2);
        assert_eq!(
            search.engine_kinds(),
            vec![SearchEngineKind::SearxNg, SearchEngineKind::DuckDuckGo]
        );
    }

    /// 验证聚合结果会轮询合并并去重。
    #[test]
    fn aggregate_results_merge_by_round_robin() {
        let first = vec![
            SearchResult {
                title: "a1".to_string(),
                url: "https://example.com/a1".to_string(),
                snippet: String::new(),
            },
            SearchResult {
                title: "a2".to_string(),
                url: "https://example.com/a2".to_string(),
                snippet: String::new(),
            },
        ];
        let second = vec![
            SearchResult {
                title: "b1".to_string(),
                url: "https://example.com/b1".to_string(),
                snippet: String::new(),
            },
            SearchResult {
                title: "a2 duplicate".to_string(),
                url: "https://example.com/a2".to_string(),
                snippet: String::new(),
            },
        ];

        let results = merge_result_groups(vec![first, second], 10);

        assert_eq!(
            results
                .iter()
                .map(|result| result.title.as_str())
                .collect::<Vec<_>>(),
            vec!["a1", "b1", "a2"]
        );
    }

    /// live 验证聚合搜索可以从真实页面返回结果。
    #[tokio::test]
    #[ignore = "live 测试需要访问搜索引擎"]
    async fn live_aggregate_search_returns_results() {
        let search = AggregateSearch::builder().limit(20).build().unwrap();

        let response = search.search("rust programming language").await.unwrap();

        assert!(!response.results.is_empty());
        assert!(response.results.len() <= 20);
    }
}
