//! Llpha 是一个轻量级 Rust 爬虫与数据分析套件。

pub mod analysis;
pub mod config;
pub mod downloader;
pub mod engine;
pub mod fake;
pub mod logging;
pub mod plugin;
pub mod websearch;

pub use analysis::*;
pub use config::*;
pub use downloader::*;
pub use engine::*;
pub use fake::*;
pub use logging::*;
pub use plugin::*;
pub use websearch::*;

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

/// user_agents 保留旧版 User-Agent 模块路径的兼容导出。
pub mod user_agents {
    pub use crate::fake::user_agents::*;
}
