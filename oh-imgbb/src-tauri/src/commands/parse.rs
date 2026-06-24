//! parse 命令负责解析 ImgBB 相册和个人空间。

use anyhow::Result;
use imgbb::ibb_spider::{IbbAlbumDetail, IbbProfileBatch, IbbProfileReport, IbbSpiderManager};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State, Window};

use crate::app_state::AppState;
use crate::db::{models::CachedResponse, repository};
use crate::settings::AppSettings;
use crate::thumbnail_cache;

/// ProfileDetail 保存个人空间解析结果。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProfileDetail {
    pub url: String,
    pub albums: Vec<imgbb::ibb_spider::IbbProfileAlbum>,
}

/// 将 anyhow 错误转换为 Tauri 可序列化错误。
fn command_result<T>(result: Result<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

/// 按设置为相册补齐本地缩略图缓存。
async fn cache_album_thumbnails_if_enabled(
    state: &AppState,
    detail: &mut IbbAlbumDetail,
) -> Result<()> {
    let settings = repository::load_settings(&state.db_pool)
        .await?
        .unwrap_or_else(|| AppSettings::with_download_dir(&state.default_download_dir));
    if !settings.thumbnail_cache_enabled {
        return Ok(());
    }

    thumbnail_cache::cache_album_thumbnails(
        detail,
        &state.thumbnail_cache_dir,
        settings.thumbnail_cache_limit_mb,
    )
    .await
}

/// 解析相册并写入缓存。
#[tauri::command]
pub async fn parse_album(
    state: State<'_, AppState>,
    url: String,
    refresh: bool,
) -> Result<CachedResponse<IbbAlbumDetail>, String> {
    command_result(
        async {
            let normalized_url = IbbSpiderManager::normalize_album_url(&url)?;
            if !refresh {
                if let Some(mut record) =
                    repository::load_album_cache(&state.db_pool, &normalized_url).await?
                {
                    cache_album_thumbnails_if_enabled(&state, &mut record.detail).await?;
                    return Ok(CachedResponse {
                        data: record.detail,
                        cached: true,
                        parsed_at: record.parsed_at,
                    });
                }
            }

            let mut detail = IbbSpiderManager::new().parse_album(&normalized_url).await?;
            cache_album_thumbnails_if_enabled(&state, &mut detail).await?;
            let parsed_at = repository::save_album_cache(&state.db_pool, &detail).await?;

            Ok(CachedResponse {
                data: detail,
                cached: false,
                parsed_at,
            })
        }
        .await,
    )
}

/// 解析个人空间并流式推送专辑批次。
#[tauri::command]
pub async fn parse_profile(
    window: Window,
    state: State<'_, AppState>,
    url: String,
    refresh: bool,
) -> Result<CachedResponse<ProfileDetail>, String> {
    command_result(
        async {
            let normalized_url = IbbSpiderManager::normalize_profile_url(&url)?;
            if !refresh {
                if let Some(record) =
                    repository::load_profile_cache(&state.db_pool, &normalized_url).await?
                {
                    let detail = ProfileDetail {
                        url: normalized_url.clone(),
                        albums: record.report.albums,
                    };

                    return Ok(CachedResponse {
                        data: detail,
                        cached: true,
                        parsed_at: record.parsed_at,
                    });
                }
            }

            let event_window = window.clone();
            let report: IbbProfileReport = IbbSpiderManager::new()
                .stream_profile_albums(&normalized_url, move |batch: IbbProfileBatch| {
                    let event_window = event_window.clone();

                    async move {
                        event_window.emit("profile://album_found", batch)?;
                        Ok(())
                    }
                })
                .await?;
            let parsed_at =
                repository::save_profile_cache(&state.db_pool, &normalized_url, &report).await?;
            let detail = ProfileDetail {
                url: normalized_url,
                albums: report.albums,
            };

            Ok(CachedResponse {
                data: detail,
                cached: false,
                parsed_at,
            })
        }
        .await,
    )
}
