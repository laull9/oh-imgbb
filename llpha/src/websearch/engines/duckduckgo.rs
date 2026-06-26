use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Url;
use scraper::{ElementRef, Html};

use crate::downloader::LlphaClient;

use crate::websearch::config::{SearchBuilder, SearchConfig};
use crate::websearch::core::{
    SearchEngine, SearchProvider, absolute_url, normalize_space, parse_selector, run_ping,
    run_search,
};
use crate::websearch::types::{
    SearchEngineKind, SearchPage, SearchPing, SearchResponse, SearchResult,
};

/// DEFAULT_DUCKDUCKGO_BASE_URL 表示 DuckDuckGo HTML 搜索默认地址。
pub const DEFAULT_DUCKDUCKGO_BASE_URL: &str = "https://duckduckgo.com/html/";

/// DuckDuckGoSearchBuilder 构建 DuckDuckGo 搜索引擎。
#[derive(Clone)]
pub struct DuckDuckGoSearchBuilder {
    builder: SearchBuilder,
    client: Option<Arc<LlphaClient>>,
}

impl Default for DuckDuckGoSearchBuilder {
    /// 创建默认 DuckDuckGo 搜索构建器。
    fn default() -> Self {
        Self {
            builder: SearchBuilder::new(DEFAULT_DUCKDUCKGO_BASE_URL),
            client: None,
        }
    }
}

impl DuckDuckGoSearchBuilder {
    /// 创建 DuckDuckGo 搜索构建器。
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

    /// 构建 DuckDuckGo 搜索引擎。
    pub fn build(self) -> Result<DuckDuckGoSearch> {
        let client = match self.client {
            Some(client) => client,
            None => Arc::new(LlphaClient::builder().without_fake().build()?),
        };

        Ok(DuckDuckGoSearch {
            config: self.builder.build()?,
            client,
        })
    }
}

/// DuckDuckGoSearch 提供 DuckDuckGo HTML 页面解析搜索能力。
pub struct DuckDuckGoSearch {
    config: SearchConfig,
    client: Arc<LlphaClient>,
}

impl DuckDuckGoSearch {
    /// 创建默认 DuckDuckGo 搜索引擎。
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// 创建 DuckDuckGo 搜索构建器。
    pub fn builder() -> DuckDuckGoSearchBuilder {
        DuckDuckGoSearchBuilder::new()
    }
}

#[async_trait]
impl SearchEngine for DuckDuckGoSearch {
    /// 返回 DuckDuckGo 搜索引擎类型。
    fn kind(&self) -> SearchEngineKind {
        SearchEngineKind::DuckDuckGo
    }

    /// 执行 DuckDuckGo 搜索并返回结构化结果。
    async fn search(&self, query: &str) -> Result<SearchResponse> {
        run_search(self, query).await
    }

    /// 快速探测 DuckDuckGo 搜索是否可用。
    async fn ping(&self) -> Result<SearchPing> {
        run_ping(self).await
    }
}

impl SearchProvider for DuckDuckGoSearch {
    /// 返回 DuckDuckGo 搜索引擎类型。
    fn kind(&self) -> SearchEngineKind {
        SearchEngineKind::DuckDuckGo
    }

    /// 返回 DuckDuckGo 搜索配置。
    fn config(&self) -> &SearchConfig {
        &self.config
    }

    /// 返回 DuckDuckGo 搜索 HTTP 客户端。
    fn client(&self) -> Arc<LlphaClient> {
        self.client.clone()
    }

    /// 构造 DuckDuckGo 搜索页面地址。
    fn page_url(
        &self,
        base_url: &str,
        query: &str,
        _page_index: usize,
        offset: usize,
    ) -> Result<String> {
        let mut url = Url::parse(base_url)
            .with_context(|| format!("解析 DuckDuckGo base_url 失败: {base_url}"))?;
        if url.path() == "/" {
            url.set_path("/html/");
        }

        url.query_pairs_mut()
            .clear()
            .append_pair("q", query)
            .append_pair("s", &offset.to_string());

        Ok(url.to_string())
    }

    /// 解析 DuckDuckGo 搜索页面内容。
    fn parse_page(&self, html: &str, page_url: &str) -> Result<SearchPage> {
        parse_duckduckgo_page(html, page_url)
    }
}

/// 解析 DuckDuckGo 搜索页面内容。
fn parse_duckduckgo_page(html: &str, page_url: &str) -> Result<SearchPage> {
    let document = Html::parse_document(html);
    let result_selector = parse_selector(".result")?;
    let title_selector = parse_selector("a.result__a, h2 a")?;
    let snippet_selector = parse_selector(".result__snippet, .result__body")?;
    let next_selector = parse_selector("a.result--more__btn, a[rel='next']")?;
    let mut results = Vec::new();

    for element in document.select(&result_selector) {
        let Some(link) = element.select(&title_selector).next() else {
            continue;
        };
        let title = normalize_space(&link.text().collect::<Vec<_>>().join(" "));
        let Some(raw_url) = link
            .value()
            .attr("href")
            .and_then(|href| absolute_url(page_url, href))
        else {
            continue;
        };
        let url = clean_duckduckgo_url(&raw_url);

        if title.is_empty() || !is_http_url(&url) || is_duckduckgo_ad_url(&url) {
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
        .and_then(|href| absolute_url(page_url, href))
        .or_else(|| parse_next_form(&document, page_url));

    Ok(SearchPage::new(results, next_url))
}

/// 从 DuckDuckGo 跳转地址中还原目标地址。
fn clean_duckduckgo_url(raw_url: &str) -> String {
    Url::parse(raw_url)
        .ok()
        .and_then(|url| {
            url.query_pairs()
                .find(|(key, _)| key == "uddg")
                .map(|(_, value)| value.into_owned())
        })
        .unwrap_or_else(|| raw_url.to_string())
}

/// 解析 DuckDuckGo 下一页表单地址。
fn parse_next_form(document: &Html, page_url: &str) -> Option<String> {
    let form_selector = parse_selector("form").ok()?;
    let input_selector = parse_selector("input").ok()?;

    document.select(&form_selector).find_map(|form| {
        let action = form.value().attr("action").unwrap_or("/html/");
        let mut url = Url::parse(page_url).ok()?.join(action).ok()?;
        let inputs = collect_form_inputs(form, &input_selector);

        if !inputs
            .iter()
            .any(|(name, _)| name == "s" || name == "nextParams")
        {
            return None;
        }

        url.query_pairs_mut().clear().extend_pairs(inputs);
        Some(url.to_string())
    })
}

/// 收集 DuckDuckGo 下一页表单参数。
fn collect_form_inputs(
    form: ElementRef<'_>,
    input_selector: &scraper::Selector,
) -> Vec<(String, String)> {
    form.select(input_selector)
        .filter_map(|input| {
            let name = input.value().attr("name")?.to_string();
            let value = input.value().attr("value").unwrap_or_default().to_string();
            Some((name, value))
        })
        .collect()
}

/// 判断地址是否为 HTTP 结果地址。
fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// 判断地址是否为 DuckDuckGo 广告跳转地址。
fn is_duckduckgo_ad_url(url: &str) -> bool {
    Url::parse(url).ok().is_some_and(|url| {
        url.domain() == Some("duckduckgo.com") && url.path().eq_ignore_ascii_case("/y.js")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 DuckDuckGo 页面地址会携带查询参数。
    #[test]
    fn duckduckgo_page_url_builds_query() {
        let search = DuckDuckGoSearch::builder().limit(5).build().unwrap();
        let url =
            SearchProvider::page_url(&search, DEFAULT_DUCKDUCKGO_BASE_URL, "rust async", 0, 10)
                .unwrap();

        assert!(url.contains("q=rust+async") || url.contains("q=rust%20async"));
        assert!(url.contains("s=10"));
    }

    /// 验证 DuckDuckGo 页面解析可以提取搜索结果。
    #[test]
    fn duckduckgo_parser_extracts_results() {
        let html = r#"
            <html><body>
                <div class="result">
                    <h2><a class="result__a" href="/l/?uddg=https%3A%2F%2Fexample.com%2Frust">Rust Lang</a></h2>
                    <a class="result__snippet">Rust language site.</a>
                </div>
                <form class="next_form" action="/html/">
                    <input type="hidden" name="q" value="rust">
                    <input type="hidden" name="s" value="30">
                </form>
            </body></html>
        "#;

        let page = parse_duckduckgo_page(html, "https://duckduckgo.com/html/?q=rust").unwrap();

        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].title, "Rust Lang");
        assert_eq!(page.results[0].url, "https://example.com/rust");
        assert!(
            page.next_url
                .unwrap()
                .starts_with("https://duckduckgo.com/html/")
        );
    }

    /// 验证 DuckDuckGo 页面解析会跳过广告跳转结果。
    #[test]
    fn duckduckgo_parser_skips_ad_results() {
        let html = r#"
            <html><body>
                <div class="result">
                    <h2><a class="result__a" href="/y.js?u3=https%3A%2F%2Fexample.com%2Fad">Ad Result</a></h2>
                    <a class="result__snippet">ad</a>
                </div>
                <div class="result">
                    <h2><a class="result__a" href="/l/?uddg=https%3A%2F%2Fexample.com%2Forganic">Organic Result</a></h2>
                    <a class="result__snippet">organic</a>
                </div>
            </body></html>
        "#;

        let page = parse_duckduckgo_page(html, "https://duckduckgo.com/html/?q=rust").unwrap();

        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].title, "Organic Result");
    }

    /// live 验证 DuckDuckGo 搜索可以从真实页面返回结果。
    #[tokio::test]
    #[ignore = "live 测试需要访问 DuckDuckGo"]
    async fn live_duckduckgo_search_returns_results() {
        let search = DuckDuckGoSearch::builder()
            .limit(3)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap();

        let response = search.search("rust programming language").await.unwrap();

        assert!(!response.results.is_empty());
        assert!(response.results.len() <= 3);
    }
}
