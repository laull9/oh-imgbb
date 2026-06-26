//! image_detail 模块负责详情图临时下载。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::AppState;

static DETAIL_IMAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// DetailImageResponse 保存详情图临时文件路径。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DetailImageResponse {
    pub local_path: String,
}

/// 将 anyhow 错误转换为 Tauri 可序列化错误。
fn command_result<T>(result: Result<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

/// 多线程下载详情图到临时目录，不写入缓存数据库。
#[tauri::command]
pub async fn download_detail_image(
    state: State<'_, AppState>,
    url: String,
) -> Result<DetailImageResponse, String> {
    command_result(
        async {
            let path = build_detail_image_path(&state.detail_image_dir, &url)?;
            state
                .thumbnail_client
                .download_file(url, &path)
                .await
                .context("详情图下载失败")?;

            Ok(DetailImageResponse {
                local_path: path.to_string_lossy().to_string(),
            })
        }
        .await,
    )
}

/// 删除详情图临时文件，失败时保持幂等。
#[tauri::command]
pub async fn remove_detail_image(state: State<'_, AppState>, path: String) -> Result<(), String> {
    command_result(
        async {
            let path_buf = PathBuf::from(&path);
            if !path_buf.starts_with(&state.detail_image_dir) {
                anyhow::bail!("详情图临时文件路径不在允许目录内");
            }

            match tokio::fs::remove_file(&path).await {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(err).with_context(|| format!("删除详情图临时文件失败: {path}")),
            }
        }
        .await,
    )
}

/// 构造详情图临时文件路径。
fn build_detail_image_path(dir: &std::path::Path, url: &str) -> Result<PathBuf> {
    let extension = infer_url_extension(url).unwrap_or_else(|| "jpg".to_string());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("系统时间早于 Unix Epoch")?
        .as_nanos();
    let process_id = std::process::id();
    let sequence = DETAIL_IMAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);

    Ok(dir.join(format!(
        "detail-{process_id}-{now}-{sequence}-{:016x}.{extension}",
        stable_url_hash(url)
    )))
}

/// 生成 URL 的稳定哈希。
fn stable_url_hash(url: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    hasher.finish()
}

/// 根据 URL 推断图片扩展名。
fn infer_url_extension(url: &str) -> Option<String> {
    let clean_url = url.split(['?', '#']).next().unwrap_or(url);
    let filename = clean_url.rsplit('/').next()?;
    let extension = filename.rsplit_once('.')?.1.to_ascii_lowercase();
    if extension.is_empty()
        || extension.len() > 8
        || !extension.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return None;
    }

    Some(extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证详情图路径包含扩展名且每次唯一。
    #[test]
    fn detail_image_path_keeps_extension_and_is_unique() {
        let dir = PathBuf::from("/tmp");
        let first = build_detail_image_path(&dir, "https://i.ibb.co/demo/photo.png?x=1").unwrap();
        let second = build_detail_image_path(&dir, "https://i.ibb.co/demo/photo.png?x=1").unwrap();

        assert_eq!(
            first.extension().and_then(|value| value.to_str()),
            Some("png")
        );
        assert_ne!(first, second);
    }
}
