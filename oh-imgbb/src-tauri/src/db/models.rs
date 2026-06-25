//! models 模块定义数据库和前端共享的数据结构。

use serde::{Deserialize, Serialize};

/// FavoriteRecord 保存一条收藏记录。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FavoriteRecord {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub url: String,
    pub cover_url: Option<String>,
    pub local_cover_path: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// FavoriteInput 保存新增收藏的输入。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FavoriteInput {
    pub kind: String,
    pub title: String,
    pub url: String,
    pub cover_url: Option<String>,
    pub note: Option<String>,
}

/// CachedResponse 为解析结果补充缓存状态。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CachedResponse<T> {
    pub data: T,
    pub cached: bool,
    pub parsed_at: String,
}

/// DownloadReport 保存下载命令的摘要。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DownloadReport {
    pub normalized_url: String,
    pub author_url: Option<String>,
    pub directory: String,
    pub downloaded_files: usize,
    pub bytes_written: usize,
}

/// ParseTabRecord 保存一个可恢复的解析结果标签页。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParseTabRecord {
    pub tab_key: String,
    pub kind: String,
    pub title: String,
    pub url: String,
    pub sort_index: i64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// ParseTabInput 保存解析标签页写入参数。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParseTabInput {
    pub tab_key: String,
    pub kind: String,
    pub title: String,
    pub url: String,
    pub sort_index: i64,
    pub active: bool,
}
