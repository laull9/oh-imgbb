//! thumbnail_cache 模块负责缩略图本地缓存。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use imgbb::ibb_spider::IbbAlbumDetail;
use llpha::LlphaClient;
use tokio::fs;
use tracing::warn;

/// CacheFile 保存待清理缓存文件的元信息。
struct CacheFile {
    path: PathBuf,
    size: u64,
    modified: std::time::SystemTime,
}

/// 为相册图片补齐本地缩略图路径。
pub async fn cache_album_thumbnails(
    detail: &mut IbbAlbumDetail,
    cache_dir: &Path,
    limit_mb: usize,
) -> Result<()> {
    fs::create_dir_all(cache_dir)
        .await
        .with_context(|| format!("创建缩略图缓存目录失败: {}", cache_dir.display()))?;

    let client = LlphaClient::global();
    for image in &mut detail.images {
        let Some(thumbnail_url) = image.thumbnail_url.clone() else {
            continue;
        };

        let path = cache_dir.join(thumbnail_file_name(&thumbnail_url));
        if !is_existing_file_usable(&path).await? {
            match client.download_file(thumbnail_url.clone(), &path).await {
                Ok(_) => {}
                Err(err) => {
                    warn!(url = %thumbnail_url, error = %err, "缩略图缓存下载失败");
                    continue;
                }
            }
        }

        image.local_thumbnail_path = Some(path.to_string_lossy().to_string());
    }

    prune_thumbnail_cache(cache_dir, limit_mb).await
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
