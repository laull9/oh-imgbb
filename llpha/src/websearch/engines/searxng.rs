use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::{Url, header::ACCEPT};
use serde::Deserialize;
use tokio::task::JoinSet;

use crate::downloader::{
    FetchRequest, LlphaClient, RetryPolicy, browser_page_headers, insert_header,
};

use crate::websearch::config::{SearchBuilder, SearchConfig};
use crate::websearch::core::{
    SearchEngine, SearchProvider, normalize_space, run_search_with_base_urls,
};
use crate::websearch::types::{
    SearchEngineKind, SearchPage, SearchPing, SearchResponse, SearchResult,
};

pub use super::searxng_mirrors::{
    DEFAULT_SEARXNG_BASE_URL, DEFAULT_SEARXNG_FALLBACK_BASE_URL, DEFAULT_SEARXNG_MIRRORS,
};
use super::searxng_mirrors::{DEFAULT_SEARXNG_PRESELECT_GOAL, DEFAULT_SEARXNG_PRESELECT_LIMIT};

/// SearxNgSearchBuilder 构建 SearxNG 搜索引擎。
#[derive(Clone)]
pub struct SearxNgSearchBuilder {
    builder: SearchBuilder,
    client: Option<Arc<LlphaClient>>,
}

impl Default for SearxNgSearchBuilder {
    /// 创建默认 SearxNG 搜索构建器。
    fn default() -> Self {
        let mut builder = SearchBuilder::new(DEFAULT_SEARXNG_BASE_URL);
        for mirror in DEFAULT_SEARXNG_MIRRORS.iter().skip(1) {
            builder = builder.fallback_base_url(*mirror);
        }

        Self {
            builder,
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
            None => Arc::new(
                LlphaClient::builder()
                    .retry_policy(RetryPolicy::new(0))
                    .without_fake()
                    .build()?,
            ),
        };

        Ok(SearxNgSearch {
            config: self.builder.build()?,
            client,
        })
    }
}

/// SearxNgSearch 提供 SearXNG API 搜索能力。
#[derive(Clone)]
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

    /// 快速预选单个搜索镜像。
    async fn preselect_base_urls(&self) -> Vec<String> {
        if self.config.base_urls.len() <= 1 {
            return self.config.base_urls.clone();
        }

        let pings = self
            .race_ping_candidates(
                DEFAULT_SEARXNG_PRESELECT_LIMIT,
                Some(DEFAULT_SEARXNG_PRESELECT_GOAL),
            )
            .await;
        if let Some(ping) = pings
            .iter()
            .filter(|ping| ping.available)
            .map(|ping| ping.base_url.clone())
            .next()
        {
            return vec![ping];
        }

        vec![self.config.base_urls[0].clone()]
    }

    /// 并发探测候选镜像并按可用性和延迟排序。
    async fn race_ping_candidates(
        &self,
        max_candidates: usize,
        stop_after_available: Option<usize>,
    ) -> Vec<SearchPing> {
        let mut join_set = JoinSet::new();
        for (index, base_url) in self
            .config
            .base_urls
            .iter()
            .take(max_candidates)
            .enumerate()
        {
            let search = self.clone();
            let base_url = base_url.clone();
            join_set.spawn(async move { (index, search.ping_candidate(&base_url).await) });
        }

        let mut pings = Vec::new();
        while let Some(output) = join_set.join_next().await {
            let Ok((index, ping)) = output else {
                continue;
            };
            pings.push((index, ping));

            let available_count = pings.iter().filter(|(_, ping)| ping.available).count();
            if stop_after_available.is_some_and(|goal| available_count >= goal) {
                join_set.abort_all();
                break;
            }
        }

        sort_ping_results(pings)
    }

    /// 探测单个 SearxNG 镜像是否可提供 JSON API。
    async fn ping_candidate(&self, base_url: &str) -> SearchPing {
        let started_at = Instant::now();
        let page_url = match self.page_url(base_url, "ping", 0, 0) {
            Ok(page_url) => page_url,
            Err(err) => {
                return SearchPing::single(
                    SearchEngineKind::SearxNg,
                    false,
                    base_url.to_string(),
                    None,
                    started_at.elapsed().as_millis(),
                    Some(err.to_string()),
                );
            }
        };

        let request = match self.ping_request(base_url, &page_url) {
            Ok(request) => request,
            Err(err) => {
                return SearchPing::single(
                    SearchEngineKind::SearxNg,
                    false,
                    base_url.to_string(),
                    None,
                    started_at.elapsed().as_millis(),
                    Some(err.to_string()),
                );
            }
        };

        let response = match self.client.fetch(request).await {
            Ok(response) => response,
            Err(err) => {
                return SearchPing::single(
                    SearchEngineKind::SearxNg,
                    false,
                    base_url.to_string(),
                    None,
                    started_at.elapsed().as_millis(),
                    Some(err.to_string()),
                );
            }
        };

        let latency_ms = started_at.elapsed().as_millis();
        let status = response.status.as_u16();
        if status != 200 {
            return SearchPing::single(
                SearchEngineKind::SearxNg,
                false,
                base_url.to_string(),
                Some(status),
                latency_ms,
                Some(format!("SearxNG ping 返回异常状态码: {status}")),
            );
        }

        match self.parse_page(&response.body, &response.url) {
            Ok(_) => SearchPing::single(
                SearchEngineKind::SearxNg,
                true,
                base_url.to_string(),
                Some(status),
                latency_ms,
                None,
            ),
            Err(err) => SearchPing::single(
                SearchEngineKind::SearxNg,
                false,
                base_url.to_string(),
                Some(status),
                latency_ms,
                Some(err.to_string()),
            ),
        }
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
        let base_urls = self.preselect_base_urls().await;
        run_search_with_base_urls(self, query, &base_urls).await
    }

    /// 快速探测 SearxNG 搜索是否可用。
    async fn ping(&self) -> Result<SearchPing> {
        self.config.validate()?;

        let children = self
            .race_ping_candidates(
                DEFAULT_SEARXNG_PRESELECT_LIMIT,
                Some(DEFAULT_SEARXNG_PRESELECT_GOAL),
            )
            .await;
        Ok(SearchPing::group(SearchEngineKind::SearxNg, children))
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
            .append_pair("format", "json")
            .append_pair("safesearch", "0")
            .append_pair("pageno", &(page_index + 1).to_string());

        Ok(url.to_string())
    }

    /// 解析 SearxNG API 响应内容。
    fn parse_page(&self, body: &str, page_url: &str) -> Result<SearchPage> {
        parse_searxng_api_response(body, page_url)
    }

    /// 构造 SearxNG API 搜索请求。
    fn page_request(&self, base_url: &str, page_url: &str) -> Result<FetchRequest> {
        build_searxng_api_request(base_url, page_url, self.config.timeout)
    }

    /// 构造 SearxNG API ping 请求。
    fn ping_request(&self, base_url: &str, page_url: &str) -> Result<FetchRequest> {
        build_searxng_api_request(
            base_url,
            page_url,
            self.config
                .timeout
                .min(crate::websearch::config::DEFAULT_SEARCH_PING_TIMEOUT),
        )
    }
}

/// SearxNgApiResponse 表示 SearXNG JSON API 响应。
#[derive(Debug, Default, Deserialize)]
struct SearxNgApiResponse {
    #[serde(default)]
    results: Vec<SearxNgApiResult>,
    #[serde(default)]
    unresponsive_engines: Vec<serde_json::Value>,
}

/// SearxNgApiResult 表示 SearXNG JSON API 单条结果。
#[derive(Debug, Default, Deserialize)]
struct SearxNgApiResult {
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
}

/// 解析 SearxNG API 响应内容。
fn parse_searxng_api_response(body: &str, page_url: &str) -> Result<SearchPage> {
    if is_rate_limited_response(body) {
        return Err(anyhow!("SearxNG 镜像返回限流响应"));
    }

    let response: SearxNgApiResponse = serde_json::from_str(body)
        .with_context(|| format!("解析 SearxNG JSON 响应失败: {page_url}"))?;

    let mut results = Vec::new();
    for item in response.results {
        let Some(result) = convert_searxng_api_result(item, page_url) else {
            continue;
        };
        results.push(result);
    }

    if results.is_empty() && !response.unresponsive_engines.is_empty() {
        return Err(anyhow!(
            "SearxNG 镜像没有返回可解析结果，部分搜索引擎无响应"
        ));
    }

    Ok(SearchPage::new(results, None))
}

/// 转换 SearxNG API 单条结果。
fn convert_searxng_api_result(item: SearxNgApiResult, page_url: &str) -> Option<SearchResult> {
    let url = normalize_space(item.url?.as_str());
    if !is_http_url(&url) || is_internal_url(page_url, &url) {
        return None;
    }

    let title = item
        .title
        .as_deref()
        .map(normalize_space)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| url.clone());
    let snippet = item
        .content
        .as_deref()
        .map(normalize_space)
        .unwrap_or_default();

    Some(SearchResult {
        title,
        url,
        snippet,
    })
}

/// 构造 SearxNG JSON API 搜索请求。
fn build_searxng_api_request(
    base_url: &str,
    page_url: &str,
    timeout: std::time::Duration,
) -> Result<FetchRequest> {
    let url =
        Url::parse(page_url).with_context(|| format!("解析 SearxNG 搜索地址失败: {page_url}"))?;
    let mut headers = browser_page_headers(base_url)?;
    insert_header(&mut headers, ACCEPT, "application/json")?;

    Ok(FetchRequest::get(url.to_string())
        .with_headers(headers)
        .with_timeout(timeout))
}

/// 判断响应是否为镜像限流响应。
fn is_rate_limited_response(body: &str) -> bool {
    normalize_space(body).eq_ignore_ascii_case("Too Many Requests")
        || body.contains("HTTP 429")
        || body.to_ascii_lowercase().contains("rate limit")
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

/// 排序 ping 结果并隐藏内部调度序号。
fn sort_ping_results(mut pings: Vec<(usize, SearchPing)>) -> Vec<SearchPing> {
    pings.sort_by(|(left_index, left), (right_index, right)| {
        right
            .available
            .cmp(&left.available)
            .then(left.latency_ms.cmp(&right.latency_ms))
            .then(left_index.cmp(right_index))
    });

    pings.into_iter().map(|(_, ping)| ping).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    /// 验证默认镜像列表包含指定实例。
    #[test]
    fn searxng_default_mirrors_include_required_instances() {
        assert!(DEFAULT_SEARXNG_MIRRORS.contains(&DEFAULT_SEARXNG_BASE_URL));
        assert!(DEFAULT_SEARXNG_MIRRORS.contains(&DEFAULT_SEARXNG_FALLBACK_BASE_URL));
        assert!(DEFAULT_SEARXNG_MIRRORS.contains(&"https://priv.au/"));
        assert!(DEFAULT_SEARXNG_MIRRORS.len() > 10);
    }

    /// 验证 ping 排序会优先返回可用且更快的镜像。
    #[test]
    fn searxng_ping_results_sort_available_first() {
        let pings = vec![
            (
                0,
                SearchPing::single(
                    SearchEngineKind::SearxNg,
                    false,
                    "https://first.example/search".to_string(),
                    Some(429),
                    10,
                    Some("limited".to_string()),
                ),
            ),
            (
                1,
                SearchPing::single(
                    SearchEngineKind::SearxNg,
                    true,
                    "https://second.example/search".to_string(),
                    Some(200),
                    20,
                    None,
                ),
            ),
            (
                2,
                SearchPing::single(
                    SearchEngineKind::SearxNg,
                    true,
                    "https://third.example/search".to_string(),
                    Some(200),
                    5,
                    None,
                ),
            ),
        ];

        let sorted = sort_ping_results(pings);

        assert_eq!(
            sorted
                .iter()
                .map(|ping| ping.base_url.as_str())
                .collect::<Vec<_>>(),
            vec![
                "https://third.example/search",
                "https://second.example/search",
                "https://first.example/search",
            ]
        );
    }

    /// 验证 SearxNG 页面地址会携带查询参数。
    #[test]
    fn searxng_page_url_builds_query() {
        let search = SearxNgSearch::builder().limit(5).build().unwrap();
        let url = SearchProvider::page_url(&search, DEFAULT_SEARXNG_BASE_URL, "rust async", 1, 10)
            .unwrap();

        assert!(url.contains("q=rust+async") || url.contains("q=rust%20async"));
        assert!(url.contains("categories=general"));
        assert!(url.contains("format=json"));
        assert!(url.contains("pageno=2"));
    }

    /// 验证 SearxNG 会构造 JSON API 请求。
    #[test]
    fn searxng_page_request_uses_json_api() {
        let search = SearxNgSearch::builder().limit(5).build().unwrap();
        let page_url =
            SearchProvider::page_url(&search, DEFAULT_SEARXNG_BASE_URL, "rust async", 0, 0)
                .unwrap();
        let request =
            SearchProvider::page_request(&search, DEFAULT_SEARXNG_BASE_URL, &page_url).unwrap();

        assert_eq!(request.method, crate::downloader::HttpMethod::Get);
        assert!(request.url.contains("q=rust+async") || request.url.contains("q=rust%20async"));
        assert!(request.url.contains("format=json"));
        assert!(request.body.is_none());
        assert_eq!(
            request
                .options
                .headers
                .get(ACCEPT)
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );
    }

    /// 验证 SearxNG API 解析可以提取搜索结果。
    #[test]
    fn searxng_api_parser_extracts_results() {
        let body = r#"
            {
              "query": "rust",
              "results": [
                {
                  "title": " Rust Lang ",
                  "url": "https://example.com/rust",
                  "content": " Rust language site. "
                }
              ],
              "unresponsive_engines": []
            }
        "#;

        let page =
            parse_searxng_api_response(body, "https://searxng.website/search?q=rust").unwrap();

        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].title, "Rust Lang");
        assert_eq!(page.results[0].url, "https://example.com/rust");
        assert_eq!(page.results[0].snippet, "Rust language site.");
        assert!(page.next_url.is_none());
    }

    /// 验证 SearxNG API 解析会跳过实例内部链接。
    #[test]
    fn searxng_api_parser_skips_internal_results() {
        let body = r#"
            {
              "results": [
                {
                  "title": "Internal",
                  "url": "https://searxng.website/search?q=rust",
                  "content": ""
                },
                {
                  "title": "",
                  "url": "https://example.com/rust",
                  "content": ""
                }
              ]
            }
        "#;

        let page =
            parse_searxng_api_response(body, "https://searxng.website/search?q=rust").unwrap();

        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].title, "https://example.com/rust");
        assert_eq!(page.results[0].url, "https://example.com/rust");
    }

    /// 验证 SearxNG HTML 首页不会被误判为 API 结果。
    #[test]
    fn searxng_api_parser_rejects_html_page() {
        let html = r#"
            <html><head><meta name="endpoint" content="index"></head>
            <body><form id="search" method="POST" action="/search"></form></body></html>
        "#;

        let err = parse_searxng_api_response(html, "https://searxng.website/search?q=rust")
            .unwrap_err()
            .to_string();

        assert!(err.contains("JSON"));
    }

    /// live 验证 SearxNG 搜索可以从真实 API 返回结果。
    #[tokio::test]
    #[ignore = "live 测试需要访问 SearxNG 镜像"]
    async fn live_searxng_search_returns_results() {
        let search = SearxNgSearch::builder()
            .limit(5)
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
            Err(err) if err.to_string().contains("403") => {
                eprintln!("SearxNG 镜像当前未启用 JSON API，跳过结果断言: {err}");
                return;
            }
            Err(err) => panic!("SearxNG live 搜索失败: {err}"),
        };

        if response.results.is_empty() {
            eprintln!("SearxNG 镜像当前没有返回可解析结果，跳过结果断言");
            return;
        }

        println!(
            "SearxNG API live 返回 {} 条结果，抓取页数 {}",
            response.results.len(),
            response.pages_fetched
        );
        for result in &response.results {
            println!("{} -> {}", result.title, result.url);
            assert!(!result.title.trim().is_empty());
            assert!(is_http_url(&result.url));
            assert!(!is_internal_url(DEFAULT_SEARXNG_BASE_URL, &result.url));
        }

        assert!(response.results.len() <= 5);
    }

    /// live 验证 SearxNG 搜索会通过真实 HTTP 请求访问 JSON API。
    #[tokio::test]
    #[ignore = "live 测试需要启动本地 HTTP API fixture"]
    async fn live_searxng_api_mode_hits_json_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}/search", listener.local_addr().unwrap());
        let (request_tx, request_rx) = oneshot::channel();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 4096];
            let read_len = socket.read(&mut buffer).await.unwrap();
            let request_text = String::from_utf8_lossy(&buffer[..read_len]).to_string();
            let _ = request_tx.send(request_text);
            let body = r#"{
                "results": [
                    {
                        "title": "Rust Programming Language",
                        "url": "https://www.rust-lang.org/",
                        "content": "A language empowering everyone."
                    }
                ],
                "unresponsive_engines": []
            }"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let search = SearxNgSearch::builder()
            .base_url(base_url.clone())
            .limit(3)
            .max_pages(1)
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        let response = search.search("rust programming language").await.unwrap();
        let request_text = request_rx.await.unwrap();

        assert!(request_text.starts_with("GET /search?"));
        assert!(request_text.contains("q=rust+programming+language"));
        assert!(request_text.contains("format=json"));
        assert!(request_text.contains("accept: application/json"));
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].title, "Rust Programming Language");
        assert_eq!(response.results[0].url, "https://www.rust-lang.org/");
        assert!(!is_internal_url(&base_url, &response.results[0].url));
    }

    /// 验证 SearxNG 搜索遇到 429 不会继续搜索后续镜像。
    #[tokio::test]
    async fn searxng_search_does_not_fallback_after_429_status() {
        let rate_limited_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let working_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let rate_limited_url = format!(
            "http://{}/search",
            rate_limited_listener.local_addr().unwrap()
        );
        let working_url = format!("http://{}/search", working_listener.local_addr().unwrap());

        tokio::spawn(async move {
            serve_rate_limited_fixture(rate_limited_listener, 2).await;
        });
        tokio::spawn(async move {
            serve_ping_blocked_search_fixture(working_listener, 1).await;
        });

        let search = SearxNgSearch::builder()
            .base_url(rate_limited_url)
            .fallback_base_url(working_url)
            .limit(3)
            .max_pages(1)
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        let err = search
            .search("rust programming language")
            .await
            .unwrap_err()
            .to_string();

        assert!(err.contains("429"));
    }

    /// 启动固定返回 429 的本地搜索 fixture。
    async fn serve_rate_limited_fixture(listener: TcpListener, request_count: usize) {
        for _ in 0..request_count {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 4096];
            let _ = socket.read(&mut buffer).await.unwrap();
            let body = "Too Many Requests";
            write_http_response(&mut socket, 429, "text/plain", body).await;
        }
    }

    /// 启动 ping 不可用但搜索可用的本地搜索 fixture。
    async fn serve_ping_blocked_search_fixture(listener: TcpListener, request_count: usize) {
        for _ in 0..request_count {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 4096];
            let read_len = socket.read(&mut buffer).await.unwrap();
            let request_text = String::from_utf8_lossy(&buffer[..read_len]);
            if request_text.contains("q=ping") {
                write_http_response(&mut socket, 403, "text/plain", "JSON API disabled").await;
                continue;
            }

            let body = r#"{
                "results": [
                    {
                        "title": "Rust Programming Language",
                        "url": "https://www.rust-lang.org/",
                        "content": "A language empowering everyone."
                    }
                ],
                "unresponsive_engines": []
            }"#;
            write_http_response(&mut socket, 200, "application/json", body).await;
        }
    }

    /// 写入简易 HTTP 响应。
    async fn write_http_response(
        socket: &mut tokio::net::TcpStream,
        status: u16,
        content_type: &str,
        body: &str,
    ) {
        let reason = match status {
            200 => "OK",
            403 => "Forbidden",
            429 => "Too Many Requests",
            _ => "Unknown",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    }
}
