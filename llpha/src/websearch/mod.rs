//! websearch 模块提供无 API 的搜索引擎页面解析能力。

mod config;
mod core;
pub mod engines;
mod types;

pub use config::{
    DEFAULT_MAX_SEARCH_PAGES, DEFAULT_SEARCH_LIMIT, DEFAULT_SEARCH_PING_TIMEOUT,
    DEFAULT_SEARCH_TIMEOUT, SearchBuilder, SearchConfig,
};
pub use core::SearchEngine;
pub use engines::{
    AggregateSearch, AggregateSearchBuilder, BingSearch, BingSearchBuilder, DEFAULT_BING_BASE_URL,
    DEFAULT_DUCKDUCKGO_BASE_URL, DEFAULT_SEARXNG_BASE_URL, DEFAULT_SEARXNG_FALLBACK_BASE_URL,
    DEFAULT_SEARXNG_MIRRORS, DuckDuckGoSearch, DuckDuckGoSearchBuilder, SearxNgSearch,
    SearxNgSearchBuilder,
};
pub use types::{SearchEngineKind, SearchPing, SearchResponse, SearchResult};
