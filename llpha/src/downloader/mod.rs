//! downloader 模块提供请求、重试、代理和客户端能力。

pub mod client;
pub mod headers;
pub mod proxy;
pub mod request;
pub mod retry;

pub use client::{DEFAULT_MAX_CONCURRENT_REQUESTS, LlphaClient, LlphaClientBuilder};
pub use headers::{
    DEFAULT_BROWSER_USER_AGENT, browser_form_headers, browser_page_headers, image_download_headers,
    insert_header,
};
pub use proxy::{InMemoryProxyPool, ProxyPool};
pub use request::{
    DownloadResponse, FetchRequest, FetchResponse, HttpMethod, RequestOptions, SavedDownload,
};
pub use retry::{DEFAULT_MAX_RETRIES, RetryPolicy};
