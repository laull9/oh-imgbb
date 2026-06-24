//! parse 命令负责解析 ImgBB 相册和个人空间。

use anyhow::Result;
use imgbb::ibb_spider::{IbbAlbumDetail, IbbProfileBatch, IbbProfileReport, IbbSpiderManager};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State, Window};

use crate::app_state::AppState;
use crate::db::{models::CachedResponse, repository};

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

/// 解析相册并写入缓存。
#[tauri::command]
pub async fn parse_album(
    state: State<'_, AppState>,
    url: String,
    refresh: bool,
) -> Result<CachedResponse<IbbAlbumDetail>, String> {
    command_result(
        async {
            if !refresh {
                if let Some(detail) = repository::load_album_cache(&state.db_pool, &url).await? {
                    return Ok(CachedResponse {
                        data: detail,
                        cached: true,
                        parsed_at: String::new(),
                    });
                }
            }

            let detail = IbbSpiderManager::new().parse_album(&url).await?;
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
            if !refresh {
                if let Some(report) = repository::load_profile_cache(&state.db_pool, &url).await? {
                    let detail = ProfileDetail {
                        url: url.clone(),
                        albums: report.albums,
                    };

                    return Ok(CachedResponse {
                        data: detail,
                        cached: true,
                        parsed_at: String::new(),
                    });
                }
            }

            let event_window = window.clone();
            let report: IbbProfileReport = IbbSpiderManager::new()
                .stream_profile_albums(&url, move |batch: IbbProfileBatch| {
                    let event_window = event_window.clone();

                    async move {
                        event_window.emit("profile://album_found", batch)?;
                        Ok(())
                    }
                })
                .await?;
            let parsed_at = repository::save_profile_cache(&state.db_pool, &url, &report).await?;
            let detail = ProfileDetail {
                url,
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
