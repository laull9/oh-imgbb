//! engines 模块保存具体搜索引擎实现。

mod aggregate;
mod bing;
mod duckduckgo;
mod searxng;

pub use aggregate::{AggregateSearch, AggregateSearchBuilder};
pub use bing::{BingSearch, BingSearchBuilder, DEFAULT_BING_BASE_URL};
pub use duckduckgo::{DEFAULT_DUCKDUCKGO_BASE_URL, DuckDuckGoSearch, DuckDuckGoSearchBuilder};
pub use searxng::{
    DEFAULT_SEARXNG_BASE_URL, DEFAULT_SEARXNG_FALLBACK_BASE_URL, DEFAULT_SEARXNG_MIRRORS,
    SearxNgSearch, SearxNgSearchBuilder,
};
