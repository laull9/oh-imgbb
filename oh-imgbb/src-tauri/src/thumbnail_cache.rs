//! thumbnail_cache 模块负责缩略图本地缓存。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use imgbb::ibb_spider::{IbbAlbumDetail, IbbAlbumImage};
use llpha::{LlphaClient, TaskPool};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::warn;

const THUMBNAIL_CACHE_CONCURRENCY: usize = 32;

/// CacheFile 保存待清理缓存文件的元信息。
struct CacheFile {
    path: PathBuf,
    size: u64,
    modified: std::time::SystemTime,
}

/// ThumbnailCacheEvent 保存单张缩略图缓存进度。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThumbnailCacheEvent {
    pub album_url: String,
    pub image_id: String,
    pub thumbnail_url: Option<String>,
    pub local_thumbnail_path: Option<String>,
    pub error: Option<String>,
}

/// ThumbnailCacheTask 保存单张缩略图缓存任务。
#[derive(Clone, Debug, Eq, PartialEq)]
struct ThumbnailCacheTask {
    album_url: String,
    image_id: String,
    thumbnail_url: String,
    path: PathBuf,
}

/// ThumbnailCacheResult 保存单张缩略图缓存结果。
#[derive(Clone, Debug, Eq, PartialEq)]
struct ThumbnailCacheResult {
    image_id: String,
    local_thumbnail_path: Option<String>,
}

/// 标记已经存在且可复用的本地缩略图。
pub async fn mark_existing_album_thumbnails(
    detail: &mut IbbAlbumDetail,
    cache_dir: &Path,
) -> Result<()> {
    fs::create_dir_all(cache_dir)
        .await
        .with_context(|| format!("创建缩略图缓存目录失败: {}", cache_dir.display()))?;

    for image in &mut detail.images {
        let Some(thumbnail_url) = image.thumbnail_url.clone() else {
            continue;
        };

        let path = cache_dir.join(thumbnail_file_name(&thumbnail_url));
        if is_existing_file_usable(&path).await? {
            image.local_thumbnail_path = Some(path.to_string_lossy().to_string());
        } else {
            image.local_thumbnail_path = None;
        }
    }

    Ok(())
}

/// 为相册图片后台缓存缩略图并按单张推送进度。
pub async fn cache_album_thumbnails_with_events<F, Fut>(
    client: Arc<LlphaClient>,
    mut detail: IbbAlbumDetail,
    cache_dir: PathBuf,
    limit_mb: usize,
    event_handler: F,
) -> Result<IbbAlbumDetail>
where
    F: Fn(ThumbnailCacheEvent) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    fs::create_dir_all(&cache_dir)
        .await
        .with_context(|| format!("创建缩略图缓存目录失败: {}", cache_dir.display()))?;

    let tasks = build_thumbnail_tasks(&detail, &cache_dir);
    let event_handler = Arc::new(event_handler);
    let report = TaskPool::new(THUMBNAIL_CACHE_CONCURRENCY)
        .run_all(tasks, move |task| {
            let client = client.clone();
            let event_handler = event_handler.clone();

            async move {
                let event = cache_thumbnail_task(client, task).await;
                let result = ThumbnailCacheResult {
                    image_id: event.image_id.clone(),
                    local_thumbnail_path: event.local_thumbnail_path.clone(),
                };
                if let Err(err) = event_handler(event).await {
                    warn!(error = %err, "缩略图缓存事件推送失败");
                }

                Ok(result)
            }
        })
        .await;

    for failure in report.failures {
        warn!(error = %failure, "缩略图缓存事件处理失败");
    }

    apply_thumbnail_results(&mut detail.images, report.successes);
    prune_thumbnail_cache(&cache_dir, limit_mb).await?;

    Ok(detail)
}

/// 构造所有需要缓存的缩略图任务。
fn build_thumbnail_tasks(detail: &IbbAlbumDetail, cache_dir: &Path) -> Vec<ThumbnailCacheTask> {
    detail
        .images
        .iter()
        .filter_map(|image| {
            let thumbnail_url = image.thumbnail_url.clone()?;
            Some(ThumbnailCacheTask {
                album_url: detail.url.clone(),
                image_id: image.id.clone(),
                path: cache_dir.join(thumbnail_file_name(&thumbnail_url)),
                thumbnail_url,
            })
        })
        .collect()
}

/// 缓存单张缩略图并转换为前端事件。
async fn cache_thumbnail_task(
    client: Arc<LlphaClient>,
    task: ThumbnailCacheTask,
) -> ThumbnailCacheEvent {
    let local_thumbnail_path = task.path.to_string_lossy().to_string();
    let result = async {
        if !is_existing_file_usable(&task.path).await? {
            client
                .download_file(task.thumbnail_url.clone(), &task.path)
                .await?;
        }

        Ok::<_, anyhow::Error>(())
    }
    .await;

    match result {
        Ok(()) => ThumbnailCacheEvent {
            album_url: task.album_url,
            image_id: task.image_id,
            thumbnail_url: Some(task.thumbnail_url),
            local_thumbnail_path: Some(local_thumbnail_path),
            error: None,
        },
        Err(err) => {
            warn!(url = %task.thumbnail_url, error = %err, "缩略图缓存下载失败");
            ThumbnailCacheEvent {
                album_url: task.album_url,
                image_id: task.image_id,
                thumbnail_url: Some(task.thumbnail_url),
                local_thumbnail_path: None,
                error: Some(err.to_string()),
            }
        }
    }
}

/// 把缩略图缓存结果写回相册详情。
fn apply_thumbnail_results(images: &mut [IbbAlbumImage], results: Vec<ThumbnailCacheResult>) {
    for result in results {
        let Some(local_thumbnail_path) = result.local_thumbnail_path else {
            continue;
        };

        if let Some(image) = images.iter_mut().find(|image| image.id == result.image_id) {
            image.local_thumbnail_path = Some(local_thumbnail_path);
        }
    }
}

/// 判断已有缓存文件是否可复用。
async fn is_existing_file_usable(path: &Path) -> Result<bool> {
    let Ok(metadata) = fs::metadata(path).await else {
        return Ok(false);
    };

    Ok(metadata.is_file() && metadata.len() > 0)
}

/// 按缓存上限清理最旧缩略图。
pub async fn prune_thumbnail_cache(cache_dir: &Path, limit_mb: usize) -> Result<()> {
    let limit_bytes = (limit_mb as u64).saturating_mul(1024).saturating_mul(1024);
    let mut files = collect_cache_files(cache_dir).await?;
    let mut total_size = files.iter().map(|file| file.size).sum::<u64>();
    if total_size <= limit_bytes {
        return Ok(());
    }

    files.sort_by_key(|file| file.modified);
    for file in files {
        if total_size <= limit_bytes {
            break;
        }

        if fs::remove_file(&file.path).await.is_ok() {
            total_size = total_size.saturating_sub(file.size);
        }
    }

    Ok(())
}

/// 清空缩略图缓存目录。
pub async fn clear_thumbnail_cache(cache_dir: &Path) -> Result<()> {
    let mut entries = fs::read_dir(cache_dir)
        .await
        .with_context(|| format!("读取缩略图缓存目录失败: {}", cache_dir.display()))?;

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if metadata.is_file() {
            fs::remove_file(entry.path()).await?;
        }
    }

    Ok(())
}

/// 收集缓存目录下的普通文件。
async fn collect_cache_files(cache_dir: &Path) -> Result<Vec<CacheFile>> {
    let mut entries = fs::read_dir(cache_dir)
        .await
        .with_context(|| format!("读取缩略图缓存目录失败: {}", cache_dir.display()))?;
    let mut files = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if !metadata.is_file() {
            continue;
        }

        files.push(CacheFile {
            path: entry.path(),
            size: metadata.len(),
            modified: metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        });
    }

    Ok(files)
}

/// 生成缩略图缓存文件名。
fn thumbnail_file_name(url: &str) -> String {
    let extension = infer_url_extension(url).unwrap_or_else(|| "jpg".to_string());
    format!("{:016x}.{extension}", stable_url_hash(url))
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

/// 对 URL 计算稳定哈希。
fn stable_url_hash(url: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in url.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证缩略图文件名使用稳定哈希和 URL 扩展名。
    #[test]
    fn thumbnail_name_uses_hash_and_extension() {
        let url = "https://i.ibb.co/demo/thumb.png?x=1";
        let name = thumbnail_file_name(url);

        assert!(name.ends_with(".png"));
        assert_eq!(name, thumbnail_file_name(url));
        assert_ne!(name, thumbnail_file_name("https://i.ibb.co/demo/other.png"));
    }

    /// 验证无法推断扩展名时使用 jpg。
    #[test]
    fn thumbnail_name_defaults_to_jpg() {
        let name = thumbnail_file_name("https://i.ibb.co/demo/no-extension");

        assert!(name.ends_with(".jpg"));
    }
}
