//! settings 模块定义应用持久化设置。

use std::path::Path;

use serde::{Deserialize, Serialize};

/// AppSettings 保存用户可配置的全局选项。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub download_dir: String,
    pub max_concurrent_downloads: usize,
    pub max_retries: usize,
    pub file_name_pattern: Option<String>,
    pub thumbnail_cache_enabled: bool,
    pub thumbnail_cache_limit_mb: usize,
    pub restore_last_page: bool,
}

impl AppSettings {
    /// 使用下载目录创建默认设置。
    pub fn with_download_dir(download_dir: &Path) -> Self {
        Self {
            download_dir: download_dir.to_string_lossy().to_string(),
            max_concurrent_downloads: 8,
            max_retries: 3,
            file_name_pattern: Some("{album}_{count}_{name}".to_string()),
            thumbnail_cache_enabled: true,
            thumbnail_cache_limit_mb: 512,
            restore_last_page: true,
        }
    }
}
