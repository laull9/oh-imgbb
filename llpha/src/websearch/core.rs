use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use scraper::Selector;

use crate::downloader::{FetchRequest, LlphaClient, browser_page_headers};

use super::config::{DEFAULT_SEARCH_PING_TIMEOUT, SearchConfig};
use super::types::{SearchEngineKind, SearchPage, SearchPing, SearchResponse};

/// SearchEngine 定义无 API 搜索引擎的异步访问接口。
///
/// 实现者负责从公开搜索页面解析结果，不依赖官方 API。默认方法提供文本和
/// JSON 两种常用返回方式，调用方也可以直接使用结构化 SearchResponse。
#[async_trait]
pub trait SearchEngine: Send + Sync {
    /// 返回当前搜索引擎类型。
    fn kind(&self) -> SearchEngineKind;

    /// 执行搜索并返回结构化结果。
    async fn search(&self, query: &str) -> Result<SearchResponse>;

    /// 快速探测搜索引擎当前是否可用。
    async fn ping(&self) -> Result<SearchPing>;

    /// 执行搜索并返回文本结果。
    async fn search_text(&self, query: &str) -> Result<String> {
        Ok(self.search(query).await?.to_text())
    }

    /// 执行搜索并返回 JSON 字符串。
    async fn search_json(&self, query: &str) -> Result<String> {
        self.search(query).await?.to_json_string()
    }
}

/// SearchProvider 定义搜索引擎页面拼装和解析能力。
pub(crate) trait SearchProvider: Send + Sync {
    /// 返回当前搜索引擎类型。
    fn kind(&self) -> SearchEngineKind;

    /// 返回搜索配置引用。
    fn config(&self) -> &SearchConfig;

    /// 返回 HTTP 客户端引用。
    fn client(&self) -> Arc<LlphaClient>;

    /// 按页码和偏移构造搜索页面地址。
    fn page_url(
        &self,
        base_url: &str,
        query: &str,
        page_index: usize,
        offset: usize,
    ) -> Result<String>;

    /// 解析搜索页面内容。
    fn parse_page(&self, html: &str, page_url: &str) -> Result<SearchPage>;

    /// 构造抓取搜索页面的请求。
    fn page_request(&self, base_url: &str, page_url: &str) -> Result<FetchRequest> {
        let headers = browser_page_headers(base_url)?;
        Ok(FetchRequest::get(page_url.to_string())
            .with_headers(headers)
            .with_timeout(self.config().timeout))
    }

    /// 构造搜索引擎可用性探测请求。
    fn ping_request(&self, base_url: &str, page_url: &str) -> Result<FetchRequest> {
        let headers = browser_page_headers(base_url)?;
        let timeout = self.config().timeout.min(DEFAULT_SEARCH_PING_TIMEOUT);
        Ok(FetchRequest::get(page_url.to_string())
            .with_headers(headers)
            .with_timeout(timeout))
    }
}

/// 执行通用搜索引擎可用性探测。
pub(crate) async fn run_ping<P>(provider: &P) -> Result<SearchPing>
where
    P: SearchProvider,
{
    provider.config().validate()?;

    let mut last_ping = None;
    for base_url in &provider.config().base_urls {
        let page_url = provider.page_url(base_url, "ping", 0, 0)?;
        let ping = ping_page(provider, base_url, &page_url).await;

        match ping {
            Ok(ping) if ping.available => return Ok(ping),
            Ok(ping) => last_ping = Some(ping),
            Err(err) => {
                last_ping = Some(SearchPing::single(
                    provider.kind(),
                    false,
                    base_url.clone(),
                    None,
                    0,
                    Some(err.to_string()),
                ));
            }
        }
    }

    Ok(last_ping.unwrap_or_else(|| {
        SearchPing::single(
            provider.kind(),
            false,
            String::new(),
            None,
            0,
            Some("搜索引擎未配置 base_url".to_string()),
        )
    }))
}

/// 执行通用搜索流程。
pub(crate) async fn run_search<P>(provider: &P, query: &str) -> Result<SearchResponse>
where
    P: SearchProvider,
{
    run_search_with_base_urls(provider, query, &provider.config().base_urls).await
}

/// 按指定源顺序执行通用搜索流程。
pub(crate) async fn run_search_with_base_urls<P>(
    provider: &P,
    query: &str,
    base_urls: &[String],
) -> Result<SearchResponse>
where
    P: SearchProvider,
{
    provider.config().validate()?;
    if base_urls.is_empty() {
        return Err(anyhow!("搜索 base_url 不能为空"));
    }

    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();
    let mut pages_fetched = 0;
    let mut next_url = None;
    let mut active_base_url = base_urls[0].clone();

    for page_index in 0..provider.config().max_pages {
        if results.len() >= provider.config().limit {
            break;
        }

        let offset = results.len();
        let page_fetch = fetch_next_page(
            provider,
            query,
            page_index,
            offset,
            &active_base_url,
            next_url,
            base_urls,
        )
        .await?;
        pages_fetched += 1;
        active_base_url = page_fetch.base_url;
        next_url = page_fetch.page.next_url;

        for result in page_fetch.page.results {
            if seen_urls.insert(result.url.clone()) {
                results.push(result);
            }

            if results.len() >= provider.config().limit {
                break;
            }
        }

        if next_url.is_none() && page_fetch.result_count == 0 {
            break;
        }
    }

    Ok(SearchResponse {
        engine: provider.kind(),
        query: query.to_string(),
        results,
        pages_fetched,
    })
}

/// PageFetch 表示一次翻页抓取的内部结果。
struct PageFetch {
    base_url: String,
    page: SearchPage,
    result_count: usize,
}

/// 抓取下一页搜索结果。
async fn fetch_next_page<P>(
    provider: &P,
    query: &str,
    page_index: usize,
    offset: usize,
    active_base_url: &str,
    next_url: Option<String>,
    base_urls: &[String],
) -> Result<PageFetch>
where
    P: SearchProvider,
{
    if let Some(next_url) = next_url {
        match fetch_page(provider, active_base_url, &next_url).await {
            Ok(page) => {
                let result_count = page.results.len();
                return Ok(PageFetch {
                    base_url: active_base_url.to_string(),
                    page,
                    result_count,
                });
            }
            Err(err) => {
                tracing::debug!("解析到的下一页请求失败，改用生成地址: {err}");
            }
        }
    }

    let mut last_error = None;
    for base_url in base_urls {
        let page_url = provider.page_url(base_url, query, page_index, offset)?;
        match fetch_page(provider, base_url, &page_url).await {
            Ok(page) => {
                let result_count = page.results.len();
                return Ok(PageFetch {
                    base_url: base_url.clone(),
                    page,
                    result_count,
                });
            }
            Err(err) => last_error = Some(err),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("搜索页面请求失败")))
}

/// 抓取并解析单个搜索页面。
async fn fetch_page<P>(provider: &P, base_url: &str, page_url: &str) -> Result<SearchPage>
where
    P: SearchProvider,
{
    let request = provider.page_request(base_url, page_url)?;
    let response = provider
        .client()
        .fetch(request)
        .await
        .with_context(|| format!("请求搜索页面失败: {page_url}"))?;

    if response.status.as_u16() != 200 {
        return Err(anyhow!("搜索页面返回异常状态码: {}", response.status));
    }

    provider.parse_page(&response.body, &response.url)
}

/// 请求单个搜索页面并返回 ping 结果。
async fn ping_page<P>(provider: &P, base_url: &str, page_url: &str) -> Result<SearchPing>
where
    P: SearchProvider,
{
    let started_at = Instant::now();
    let request = provider.ping_request(base_url, page_url)?;
    let response = provider
        .client()
        .fetch(request)
        .await
        .with_context(|| format!("请求搜索 ping 页面失败: {page_url}"))?;
    let latency_ms = started_at.elapsed().as_millis();
    let status = response.status.as_u16();

    if status != 200 {
        return Ok(SearchPing::single(
            provider.kind(),
            false,
            base_url.to_string(),
            Some(status),
            latency_ms,
            Some(format!("搜索 ping 返回异常状态码: {status}")),
        ));
    }

    match provider.parse_page(&response.body, &response.url) {
        Ok(_) => Ok(SearchPing::single(
            provider.kind(),
            true,
            base_url.to_string(),
            Some(status),
            latency_ms,
            None,
        )),
        Err(err) => Ok(SearchPing::single(
            provider.kind(),
            false,
            base_url.to_string(),
            Some(status),
            latency_ms,
            Some(err.to_string()),
        )),
    }
}

/// 解析 CSS 选择器。
pub(crate) fn parse_selector(selector: &str) -> Result<Selector> {
    Selector::parse(selector).map_err(|err| anyhow!("解析选择器失败 {selector}: {err}"))
}

/// 规整页面文本中的连续空白。
pub(crate) fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 将相对地址转换为绝对地址。
pub(crate) fn absolute_url(page_url: &str, href: &str) -> Option<String> {
    if href.trim().is_empty() || href.starts_with('#') {
        return None;
    }

    reqwest::Url::parse(page_url)
        .ok()
        .and_then(|base| base.join(href).ok())
        .map(|url| url.to_string())
}
