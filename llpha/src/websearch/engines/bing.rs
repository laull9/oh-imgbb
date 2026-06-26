use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Url;
use scraper::Html;

use crate::downloader::LlphaClient;

use crate::websearch::config::{SearchBuilder, SearchConfig};
use crate::websearch::core::{
    SearchEngine, SearchProvider, absolute_url, normalize_space, parse_selector, run_ping,
    run_search,
};
use crate::websearch::types::{
    SearchEngineKind, SearchPage, SearchPing, SearchResponse, SearchResult,
};

/// DEFAULT_BING_BASE_URL 表示 Bing 搜索默认地址。
pub const DEFAULT_BING_BASE_URL: &str = "https://www.bing.com/search";

/// BingSearchBuilder 构建 Bing 搜索引擎。
#[derive(Clone)]
pub struct BingSearchBuilder {
    builder: SearchBuilder,
    client: Option<Arc<LlphaClient>>,
}

impl Default for BingSearchBuilder {
    /// 创建默认 Bing 搜索构建器。
    fn default() -> Self {
        Self {
            builder: SearchBuilder::new(DEFAULT_BING_BASE_URL),
            client: None,
        }
    }
}

impl BingSearchBuilder {
    /// 创建 Bing 搜索构建器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置主搜索地址。
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.builder = self.builder.base_url(base_url);
        self
    }

    /// 增加搜索地址回退项。
    pub fn fallback_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.builder = self.builder.fallback_base_url(base_url);
        self
    }

    /// 设置需要返回的搜索条数。
    pub fn limit(mut self, limit: usize) -> Self {
        self.builder = self.builder.limit(limit);
        self
    }

    /// 设置单次请求超时时间。
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.builder = self.builder.timeout(timeout);
        self
    }

    /// 设置最大翻页次数。
    pub fn max_pages(mut self, max_pages: usize) -> Self {
        self.builder = self.builder.max_pages(max_pages);
        self
    }

    /// 设置自定义 HTTP 客户端。
    pub fn client(mut self, client: impl Into<Arc<LlphaClient>>) -> Self {
        self.client = Some(client.into());
        self
    }

    /// 构建 Bing 搜索引擎。
    pub fn build(self) -> Result<BingSearch> {
        let client = match self.client {
            Some(client) => client,
            None => Arc::new(LlphaClient::builder().without_fake().build()?),
        };

        Ok(BingSearch {
            config: self.builder.build()?,
            client,
        })
    }
}

/// BingSearch 提供 Bing 页面解析搜索能力。
pub struct BingSearch {
    config: SearchConfig,
    client: Arc<LlphaClient>,
}

impl BingSearch {
    /// 创建默认 Bing 搜索引擎。
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// 创建 Bing 搜索构建器。
    pub fn builder() -> BingSearchBuilder {
        BingSearchBuilder::new()
    }
}

#[async_trait]
impl SearchEngine for BingSearch {
    /// 返回 Bing 搜索引擎类型。
    fn kind(&self) -> SearchEngineKind {
        SearchEngineKind::Bing
    }

    /// 执行 Bing 搜索并返回结构化结果。
    async fn search(&self, query: &str) -> Result<SearchResponse> {
        run_search(self, query).await
    }

    /// 快速探测 Bing 搜索是否可用。
    async fn ping(&self) -> Result<SearchPing> {
        run_ping(self).await
    }
}

impl SearchProvider for BingSearch {
    /// 返回 Bing 搜索引擎类型。
    fn kind(&self) -> SearchEngineKind {
        SearchEngineKind::Bing
    }

    /// 返回 Bing 搜索配置。
    fn config(&self) -> &SearchConfig {
        &self.config
    }

    /// 返回 Bing 搜索 HTTP 客户端。
    fn client(&self) -> Arc<LlphaClient> {
        self.client.clone()
    }

    /// 构造 Bing 搜索页面地址。
    fn page_url(
        &self,
        base_url: &str,
        query: &str,
        _page_index: usize,
        offset: usize,
    ) -> Result<String> {
        let mut url =
            Url::parse(base_url).with_context(|| format!("解析 Bing base_url 失败: {base_url}"))?;
        if url.path() == "/" {
            url.set_path("/search");
        }

        url.query_pairs_mut()
            .clear()
            .append_pair("q", query)
            .append_pair("count", &self.config.limit.min(50).to_string())
            .append_pair("first", &(offset + 1).to_string());

        Ok(url.to_string())
    }

    /// 解析 Bing 搜索页面内容。
    fn parse_page(&self, html: &str, page_url: &str) -> Result<SearchPage> {
        parse_bing_page(html, page_url)
    }
}

/// 解析 Bing 搜索页面内容。
fn parse_bing_page(html: &str, page_url: &str) -> Result<SearchPage> {
    let document = Html::parse_document(html);
    let result_selector = parse_selector("li.b_algo")?;
    let title_selector = parse_selector("h2 a")?;
    let snippet_selector = parse_selector(".b_caption p, p")?;
    let next_selector =
        parse_selector("a.sb_pagN, a[title='Next page'], a[aria-label='Next page']")?;
    let mut results = Vec::new();

    for element in document.select(&result_selector) {
        let Some(link) = element.select(&title_selector).next() else {
            continue;
        };
        let title = normalize_space(&link.text().collect::<Vec<_>>().join(" "));
        let Some(url) = link
            .value()
            .attr("href")
            .and_then(|href| absolute_url(page_url, href))
        else {
            continue;
        };

        if title.is_empty() || !is_http_url(&url) {
            continue;
        }

        let snippet = element
            .select(&snippet_selector)
            .next()
            .map(|item| normalize_space(&item.text().collect::<Vec<_>>().join(" ")))
            .unwrap_or_default();

        results.push(SearchResult {
            title,
            url,
            snippet,
        });
    }

    let next_url = document
        .select(&next_selector)
        .find_map(|link| link.value().attr("href"))
        .and_then(|href| absolute_url(page_url, href));

    Ok(SearchPage::new(results, next_url))
}

/// 判断地址是否为 HTTP 结果地址。
fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 Bing 页面地址会携带查询参数。
    #[test]
    fn bing_page_url_builds_query() {
        let search = BingSearch::builder().limit(5).build().unwrap();
        let url =
            SearchProvider::page_url(&search, DEFAULT_BING_BASE_URL, "rust async", 0, 0).unwrap();

        assert!(url.contains("q=rust+async") || url.contains("q=rust%20async"));
        assert!(url.contains("count=5"));
        assert!(url.contains("first=1"));
    }

    /// 验证 Bing 页面解析可以提取搜索结果。
    #[test]
    fn bing_parser_extracts_results() {
        let html = r#"
            <html><body>
                <ol>
                    <li class="b_algo">
                        <h2><a href="https://example.com/rust">Rust Lang</a></h2>
                        <div class="b_caption"><p>Rust language site.</p></div>
                    </li>
                </ol>
                <a class="sb_pagN" href="/search?q=rust&first=11">Next</a>
            </body></html>
        "#;

        let page = parse_bing_page(html, "https://www.bing.com/search?q=rust").unwrap();

        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].title, "Rust Lang");
        assert_eq!(page.results[0].url, "https://example.com/rust");
        assert!(
            page.next_url
                .unwrap()
                .starts_with("https://www.bing.com/search")
        );
    }

    /// live 验证 Bing 搜索可以从真实页面返回结果。
    #[tokio::test]
    #[ignore = "live 测试需要访问 Bing"]
    async fn live_bing_search_returns_results() {
        let search = BingSearch::builder()
            .limit(3)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap();

        let response = search.search("rust programming language").await.unwrap();

        assert!(!response.results.is_empty());
        assert!(response.results.len() <= 3);
    }
}
