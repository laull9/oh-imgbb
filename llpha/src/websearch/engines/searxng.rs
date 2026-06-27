use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::{Url, header::CONTENT_TYPE};
use scraper::Html;

use crate::downloader::{FetchRequest, LlphaClient, browser_page_headers, insert_header};

use crate::websearch::config::{SearchBuilder, SearchConfig};
use crate::websearch::core::{
    SearchEngine, SearchProvider, absolute_url, normalize_space, parse_selector, run_ping,
    run_search,
};
use crate::websearch::types::{
    SearchEngineKind, SearchPage, SearchPing, SearchResponse, SearchResult,
};

/// DEFAULT_SEARXNG_BASE_URL 表示默认 SearxNG 搜索地址。
pub const DEFAULT_SEARXNG_BASE_URL: &str = "https://searxng.website/searxng/search";

/// DEFAULT_SEARXNG_FALLBACK_BASE_URL 表示默认 SearxNG 回退搜索地址。
pub const DEFAULT_SEARXNG_FALLBACK_BASE_URL: &str = "https://search.liuzj.net/search";

/// DEFAULT_SEARXNG_MIRRORS 表示内置 SearxNG 镜像搜索地址列表。
pub const DEFAULT_SEARXNG_MIRRORS: &[&str] =
    &[DEFAULT_SEARXNG_BASE_URL, DEFAULT_SEARXNG_FALLBACK_BASE_URL];

/// SearxNgSearchBuilder 构建 SearxNG 搜索引擎。
#[derive(Clone)]
pub struct SearxNgSearchBuilder {
    builder: SearchBuilder,
    client: Option<Arc<LlphaClient>>,
}

impl Default for SearxNgSearchBuilder {
    /// 创建默认 SearxNG 搜索构建器。
    fn default() -> Self {
        Self {
            builder: SearchBuilder::new(DEFAULT_SEARXNG_BASE_URL)
                .fallback_base_url(DEFAULT_SEARXNG_FALLBACK_BASE_URL),
            client: None,
        }
    }
}

impl SearxNgSearchBuilder {
    /// 创建 SearxNG 搜索构建器。
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

    /// 增加 SearxNG 镜像搜索地址。
    pub fn mirror(mut self, base_url: impl Into<String>) -> Self {
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

    /// 构建 SearxNG 搜索引擎。
    pub fn build(self) -> Result<SearxNgSearch> {
        let client = match self.client {
            Some(client) => client,
            None => Arc::new(LlphaClient::builder().without_fake().build()?),
        };

        Ok(SearxNgSearch {
            config: self.builder.build()?,
            client,
        })
    }
}

/// SearxNgSearch 提供 SearxNG 页面解析搜索能力。
pub struct SearxNgSearch {
    config: SearchConfig,
    client: Arc<LlphaClient>,
}

impl SearxNgSearch {
    /// 创建默认 SearxNG 搜索引擎。
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// 创建 SearxNG 搜索构建器。
    pub fn builder() -> SearxNgSearchBuilder {
        SearxNgSearchBuilder::new()
    }
}

#[async_trait]
impl SearchEngine for SearxNgSearch {
    /// 返回 SearxNG 搜索引擎类型。
    fn kind(&self) -> SearchEngineKind {
        SearchEngineKind::SearxNg
    }

    /// 执行 SearxNG 搜索并返回结构化结果。
    async fn search(&self, query: &str) -> Result<SearchResponse> {
        run_search(self, query).await
    }

    /// 快速探测 SearxNG 搜索是否可用。
    async fn ping(&self) -> Result<SearchPing> {
        run_ping(self).await
    }
}

impl SearchProvider for SearxNgSearch {
    /// 返回 SearxNG 搜索引擎类型。
    fn kind(&self) -> SearchEngineKind {
        SearchEngineKind::SearxNg
    }

    /// 返回 SearxNG 搜索配置。
    fn config(&self) -> &SearchConfig {
        &self.config
    }

    /// 返回 SearxNG 搜索 HTTP 客户端。
    fn client(&self) -> Arc<LlphaClient> {
        self.client.clone()
    }

    /// 构造 SearxNG 搜索页面地址。
    fn page_url(
        &self,
        base_url: &str,
        query: &str,
        page_index: usize,
        _offset: usize,
    ) -> Result<String> {
        let mut url = Url::parse(base_url)
            .with_context(|| format!("解析 SearxNG base_url 失败: {base_url}"))?;
        if url.path() == "/" {
            url.set_path("/search");
        }

        url.query_pairs_mut()
            .clear()
            .append_pair("q", query)
            .append_pair("categories", "general")
            .append_pair("language", "auto")
            .append_pair("pageno", &(page_index + 1).to_string());

        Ok(url.to_string())
    }

    /// 解析 SearxNG 搜索页面内容。
    fn parse_page(&self, html: &str, page_url: &str) -> Result<SearchPage> {
        parse_searxng_page(html, page_url)
    }

    /// 构造 SearxNG 表单搜索请求。
    fn page_request(&self, base_url: &str, page_url: &str) -> Result<FetchRequest> {
        build_searxng_post_request(base_url, page_url, self.config.timeout)
    }

    /// 构造 SearxNG 表单 ping 请求。
    fn ping_request(&self, base_url: &str, page_url: &str) -> Result<FetchRequest> {
        build_searxng_post_request(
            base_url,
            page_url,
            self.config
                .timeout
                .min(crate::websearch::config::DEFAULT_SEARCH_PING_TIMEOUT),
        )
    }
}

/// 解析 SearxNG 搜索页面内容。
fn parse_searxng_page(html: &str, page_url: &str) -> Result<SearchPage> {
    if is_rate_limited_page(html) {
        return Err(anyhow!("SearxNG 镜像返回限流响应"));
    }
    if is_index_page(html) {
        return Err(anyhow!("SearxNG 镜像返回首页而不是搜索结果页"));
    }

    let document = Html::parse_document(html);
    let result_selector = parse_selector("article.result, div.result")?;
    let title_selector = parse_selector("h3 a, h4 a, a.url_header")?;
    let snippet_selector = parse_selector("p.content, p.result-content, .content")?;
    let next_selector = parse_selector("a[rel='next'], a.next_page, .pagination a:last-child")?;
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

        if title.is_empty() || !is_http_url(&url) || is_internal_url(page_url, &url) {
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

    if results.is_empty() && next_url.is_none() && !is_no_results_page(html) {
        return Err(anyhow!("SearxNG 镜像没有返回可解析的搜索结果"));
    }

    Ok(SearchPage::new(results, next_url))
}

/// 构造 SearxNG POST 搜索请求。
fn build_searxng_post_request(
    base_url: &str,
    page_url: &str,
    timeout: std::time::Duration,
) -> Result<FetchRequest> {
    let mut url =
        Url::parse(page_url).with_context(|| format!("解析 SearxNG 搜索地址失败: {page_url}"))?;
    let query = url
        .query_pairs()
        .find(|(key, _)| key == "q")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default();
    let pageno = url
        .query_pairs()
        .find(|(key, _)| key == "pageno")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| "1".to_string());
    let mut body_url = Url::parse("https://llpha.local/")?;
    body_url
        .query_pairs_mut()
        .append_pair("q", &query)
        .append_pair("category_general", "1")
        .append_pair("language", "auto")
        .append_pair("time_range", "")
        .append_pair("safesearch", "0")
        .append_pair("theme", "simple")
        .append_pair("pageno", &pageno);
    let body = body_url.query().unwrap_or_default().to_string();

    let mut headers = browser_page_headers(base_url)?;
    insert_header(
        &mut headers,
        CONTENT_TYPE,
        "application/x-www-form-urlencoded",
    )?;

    url.set_query(None);

    Ok(FetchRequest::post(url.to_string(), body)
        .with_headers(headers)
        .with_timeout(timeout))
}

/// 判断页面是否为镜像限流响应。
fn is_rate_limited_page(html: &str) -> bool {
    normalize_space(html).eq_ignore_ascii_case("Too Many Requests")
        || html.contains("HTTP 429")
        || html.contains("rate limit")
}

/// 判断页面是否仍停留在 SearxNG 首页。
fn is_index_page(html: &str) -> bool {
    html.contains(r#"name="endpoint" content="index""#)
        || html.contains(r#"name="endpoint" content='index'"#)
}

/// 判断页面是否明确表示没有搜索结果。
fn is_no_results_page(html: &str) -> bool {
    html.contains("no_item_found")
        || html.contains("未找到项目")
        || html.to_ascii_lowercase().contains("no results")
}

/// 判断地址是否为 HTTP 结果地址。
fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// 判断地址是否仍指向当前 SearxNG 实例。
fn is_internal_url(page_url: &str, result_url: &str) -> bool {
    let Ok(page_url) = Url::parse(page_url) else {
        return false;
    };
    let Ok(result_url) = Url::parse(result_url) else {
        return false;
    };

    page_url.domain() == result_url.domain() && result_url.path().starts_with("/search")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证默认镜像列表包含指定实例。
    #[test]
    fn searxng_default_mirrors_include_required_instances() {
        assert!(DEFAULT_SEARXNG_MIRRORS.contains(&DEFAULT_SEARXNG_BASE_URL));
        assert!(DEFAULT_SEARXNG_MIRRORS.contains(&DEFAULT_SEARXNG_FALLBACK_BASE_URL));
    }

    /// 验证 SearxNG 页面地址会携带查询参数。
    #[test]
    fn searxng_page_url_builds_query() {
        let search = SearxNgSearch::builder().limit(5).build().unwrap();
        let url = SearchProvider::page_url(&search, DEFAULT_SEARXNG_BASE_URL, "rust async", 1, 10)
            .unwrap();

        assert!(url.contains("q=rust+async") || url.contains("q=rust%20async"));
        assert!(url.contains("categories=general"));
        assert!(url.contains("pageno=2"));
    }

    /// 验证 SearxNG 会构造表单 POST 请求。
    #[test]
    fn searxng_page_request_uses_post_form() {
        let search = SearxNgSearch::builder().limit(5).build().unwrap();
        let page_url =
            SearchProvider::page_url(&search, DEFAULT_SEARXNG_BASE_URL, "rust async", 0, 0)
                .unwrap();
        let request =
            SearchProvider::page_request(&search, DEFAULT_SEARXNG_BASE_URL, &page_url).unwrap();

        assert_eq!(request.method, crate::downloader::HttpMethod::Post);
        assert!(!request.url.contains("?q="));
        assert_eq!(
            request
                .options
                .headers
                .get(CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/x-www-form-urlencoded"
        );
        assert!(request.body.as_deref().unwrap().contains("q=rust+async"));
    }

    /// 验证 SearxNG 页面解析可以提取搜索结果。
    #[test]
    fn searxng_parser_extracts_results() {
        let html = r#"
            <html><body>
                <article class="result result-default category-general">
                    <h3><a href="https://example.com/rust">Rust Lang</a></h3>
                    <p class="content">Rust language site.</p>
                </article>
                <a rel="next" href="/search?q=rust&pageno=2">Next</a>
            </body></html>
        "#;

        let page = parse_searxng_page(html, "https://searxng.website/search?q=rust").unwrap();

        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].title, "Rust Lang");
        assert_eq!(page.results[0].url, "https://example.com/rust");
        assert!(
            page.next_url
                .unwrap()
                .starts_with("https://searxng.website/search")
        );
    }

    /// 验证 SearxNG 首页不会被误判为空搜索结果。
    #[test]
    fn searxng_parser_rejects_index_page() {
        let html = r#"
            <html><head><meta name="endpoint" content="index"></head>
            <body><form id="search" method="POST" action="/search"></form></body></html>
        "#;

        let err = parse_searxng_page(html, "https://searxng.website/search?q=rust")
            .unwrap_err()
            .to_string();

        assert!(err.contains("首页"));
    }

    /// live 验证 SearxNG 搜索可以从真实页面返回结果。
    #[tokio::test]
    #[ignore = "live 测试需要访问 SearxNG 镜像"]
    async fn live_searxng_search_returns_results() {
        let search = SearxNgSearch::builder()
            .limit(3)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap();

        let response = match search.search("rust programming language").await {
            Ok(response) => response,
            Err(err) if err.to_string().contains("限流") => {
                eprintln!("SearxNG 镜像当前限流，跳过结果断言: {err}");
                return;
            }
            Err(err) if err.to_string().contains("429") => {
                eprintln!("SearxNG 镜像当前限流，跳过结果断言: {err}");
                return;
            }
            Err(err) if err.to_string().contains("首页") => {
                eprintln!("SearxNG 镜像当前没有返回搜索页，跳过结果断言: {err}");
                return;
            }
            Err(err) => panic!("SearxNG live 搜索失败: {err}"),
        };

        if response.results.is_empty() {
            eprintln!("SearxNG 镜像当前没有返回可解析结果，跳过结果断言");
            return;
        }

        assert!(response.results.len() <= 3);
    }
}
