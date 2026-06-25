//! parse 命令负责解析 ImgBB 相册和个人空间。

use anyhow::Result;
use imgbb::ibb_spider::{IbbAlbumDetail, IbbProfileBatch, IbbProfileReport, IbbSpiderManager};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State, Window};

use crate::app_state::AppState;
use crate::db::{
    models::{CachedResponse, ParseTabInput, ParseTabRecord},
    repository,
};
use crate::settings::AppSettings;
use crate::thumbnail_cache::{self, ThumbnailCacheEvent};

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

/// 读取当前设置或返回默认设置。
async fn load_settings_or_default(state: &AppState) -> Result<AppSettings> {
    Ok(repository::load_settings(&state.db_pool)
        .await?
        .unwrap_or_else(|| AppSettings::with_download_dir(&state.default_download_dir)))
}

/// 按设置标记已经存在的本地缩略图。
async fn mark_album_thumbnails_if_enabled(
    state: &AppState,
    detail: &mut IbbAlbumDetail,
    settings: &AppSettings,
) -> Result<()> {
    if !settings.thumbnail_cache_enabled {
        return Ok(());
    }

    thumbnail_cache::mark_existing_album_thumbnails(detail, &state.thumbnail_cache_dir).await
}

/// 推送相册详情已就绪事件。
fn emit_album_detail_ready(
    window: &Window,
    detail: IbbAlbumDetail,
    cached: bool,
    parsed_at: String,
) -> Result<()> {
    window.emit(
        "album://detail_ready",
        CachedResponse {
            data: detail,
            cached,
            parsed_at,
        },
    )?;

    Ok(())
}

/// 启动后台缩略图缓存任务。
fn spawn_album_thumbnail_cache(
    window: Window,
    state: &AppState,
    detail: IbbAlbumDetail,
    settings: AppSettings,
) {
    if !settings.thumbnail_cache_enabled {
        return;
    }

    let db_pool = state.db_pool.clone();
    let cache_dir = state.thumbnail_cache_dir.clone();
    let client = state.thumbnail_client.clone();

    tauri::async_runtime::spawn(async move {
        let album_url = detail.url.clone();
        let result = thumbnail_cache::cache_album_thumbnails_with_events(
            client,
            detail,
            cache_dir,
            settings.thumbnail_cache_limit_mb,
            move |event: ThumbnailCacheEvent| {
                let event_window = window.clone();

                async move {
                    event_window.emit("album://thumbnail_cached", event)?;
                    Ok(())
                }
            },
        )
        .await;

        match result {
            Ok(detail) => {
                if let Err(err) = repository::save_album_cache(&db_pool, &detail).await {
                    tracing::warn!(url = %album_url, error = %err, "保存缩略图缓存结果失败");
                }
            }
            Err(err) => {
                tracing::warn!(url = %album_url, error = %err, "后台缩略图缓存任务失败");
            }
        }
    });
}

/// 解析相册并写入缓存。
#[tauri::command]
pub async fn parse_album(
    window: Window,
    state: State<'_, AppState>,
    url: String,
    refresh: bool,
) -> Result<CachedResponse<IbbAlbumDetail>, String> {
    command_result(
        async {
            let normalized_url = IbbSpiderManager::normalize_album_url(&url)?;
            let settings = load_settings_or_default(&state).await?;
            if !refresh {
                if let Some(mut record) =
                    repository::load_album_cache(&state.db_pool, &normalized_url).await?
                {
                    mark_album_thumbnails_if_enabled(&state, &mut record.detail, &settings).await?;
                    emit_album_detail_ready(
                        &window,
                        record.detail.clone(),
                        true,
                        record.parsed_at.clone(),
                    )?;
                    spawn_album_thumbnail_cache(window, &state, record.detail.clone(), settings);

                    return Ok(CachedResponse {
                        data: record.detail,
                        cached: true,
                        parsed_at: record.parsed_at,
                    });
                }
            }

            let mut detail = IbbSpiderManager::new().parse_album(&normalized_url).await?;
            mark_album_thumbnails_if_enabled(&state, &mut detail, &settings).await?;
            let parsed_at = repository::save_album_cache(&state.db_pool, &detail).await?;
            emit_album_detail_ready(&window, detail.clone(), false, parsed_at.clone())?;
            spawn_album_thumbnail_cache(window, &state, detail.clone(), settings);

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

/// 读取可恢复的解析标签页列表。
#[tauri::command]
pub async fn list_parse_tabs(state: State<'_, AppState>) -> Result<Vec<ParseTabRecord>, String> {
    command_result(async { repository::list_parse_tabs(&state.db_pool).await }.await)
}

/// 保存或更新解析标签页。
#[tauri::command]
pub async fn save_parse_tab(state: State<'_, AppState>, tab: ParseTabInput) -> Result<(), String> {
    command_result(async { repository::save_parse_tab(&state.db_pool, &tab).await }.await)
}

/// 删除解析标签页。
#[tauri::command]
pub async fn remove_parse_tab(state: State<'_, AppState>, tab_key: String) -> Result<(), String> {
    command_result(async { repository::remove_parse_tab(&state.db_pool, &tab_key).await }.await)
}

/// 设置当前激活的解析标签页。
#[tauri::command]
pub async fn set_active_parse_tab(
    state: State<'_, AppState>,
    tab_key: Option<String>,
) -> Result<(), String> {
    command_result(
        async { repository::set_active_parse_tab(&state.db_pool, tab_key.as_deref()).await }.await,
    )
}
