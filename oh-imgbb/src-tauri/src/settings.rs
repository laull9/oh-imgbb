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
    #[serde(default)]
    pub imgbb_login_subject: Option<String>,
    #[serde(default)]
    pub imgbb_password: Option<String>,
    pub thumbnail_cache_enabled: bool,
    pub thumbnail_cache_limit_mb: usize,
    pub restore_last_page: bool,
    #[serde(default = "default_pagination_enabled")]
    pub pagination_enabled: bool,
    #[serde(default = "default_profile_page_size")]
    pub profile_page_size: usize,
    #[serde(default = "default_album_page_size")]
    pub album_page_size: usize,
}

impl AppSettings {
    /// 使用下载目录创建默认设置。
    pub fn with_download_dir(download_dir: &Path) -> Self {
        Self {
            download_dir: download_dir.to_string_lossy().to_string(),
            max_concurrent_downloads: 8,
            max_retries: 3,
            file_name_pattern: Some("{album}_{count}_{name}".to_string()),
            imgbb_login_subject: None,
            imgbb_password: None,
            thumbnail_cache_enabled: true,
            thumbnail_cache_limit_mb: 64,
            restore_last_page: true,
            pagination_enabled: default_pagination_enabled(),
            profile_page_size: default_profile_page_size(),
            album_page_size: default_album_page_size(),
        }
    }
}

/// 默认开启解析结果分页。
fn default_pagination_enabled() -> bool {
    true
}

/// 默认个人空间相册每页数量。
fn default_profile_page_size() -> usize {
    10
}

/// 默认相册图片每页数量。
fn default_album_page_size() -> usize {
    20
}
