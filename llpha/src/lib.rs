//! Llpha 是一个轻量级 Rust 爬虫与数据分析套件。

pub mod analysis;
pub mod config;
pub mod downloader;
pub mod engine;
pub mod fake;
pub mod logging;
pub mod plugin;

pub use analysis::{
    ExtractResult, HtmlElement, HtmlExtractor, HtmlQuery, Link, html_attr, html_attrs,
    html_fragment_attr, html_fragment_attrs, html_fragment_text, html_fragment_texts, html_text,
    html_texts, required_json_array, required_json_item_string, required_json_string,
};
pub use config::{
    AppConfig, DEFAULT_CONFIG_PATH, LoggingConfig, LoggingFormat, LoggingTarget, RequestConfig,
    load_config,
};
pub use downloader::{
    DEFAULT_BROWSER_USER_AGENT, DEFAULT_MAX_CONCURRENT_REQUESTS, DEFAULT_MAX_RETRIES,
    DownloadResponse, FetchRequest, FetchResponse, HttpMethod, InMemoryProxyPool, LlphaClient,
    LlphaClientBuilder, ProxyPool, RequestOptions, RetryPolicy, SavedDownload,
    browser_form_headers, browser_page_headers, image_download_headers, insert_header,
};
pub use engine::{
    Action, ActionGenerator, ActionKind, EngineReport, Fetcher, FnFetcher, HtmlParser,
    InMemoryScheduler, LinkActionGenerator, LlphaFetcher, Page, PageBody, Parser, Scheduler, Task,
    TaskEdge, TaskGraph, TaskId, TaskKind, TaskNode, TaskPool, TaskPoolReport, TaskStatus,
    UNASSIGNED_TASK_ID, WorkflowEngine, WorkflowEngineBuilder,
};
pub use fake::{
    FakeProfile, HeaderFakeStrategy, RandomHeaderFakeStrategy, StaticHeaderFakeStrategy,
    browser_profiles,
};
pub use logging::{LoggingGuard, init_logging};
pub use plugin::{Plugin, PluginContext, PluginRegistry};

/// client 保留旧版客户端模块路径的兼容导出。
pub mod client {
    pub use crate::downloader::client::*;
}

/// extract 保留旧版提取模块路径的兼容导出。
pub mod extract {
    pub use crate::analysis::extract::*;
}

/// proxy 保留旧版代理模块路径的兼容导出。
pub mod proxy {
    pub use crate::downloader::proxy::*;
}

/// request 保留旧版请求模块路径的兼容导出。
pub mod request {
    pub use crate::downloader::request::*;
}

/// retry 保留旧版重试模块路径的兼容导出。
pub mod retry {
    pub use crate::downloader::retry::*;
}
