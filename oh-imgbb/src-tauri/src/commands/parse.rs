//! parse 命令负责解析 ImgBB 相册和个人空间。

use std::collections::HashSet;

use anyhow::Result;
use imgbb::ibb_spider::{
    IbbAlbumDetail, IbbLoginSession, IbbProfileBatch, IbbProfileReport, IbbSpiderManager,
};
use llpha::{AggregateSearch, SearchEngine, SearchPing, SearchResult};
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

/// SearchAlbumsDetail 保存网络搜索提取到的 ImgBB 相册列表。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchAlbumsDetail {
    pub query: String,
    pub search_query: String,
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

/// 读取匹配当前登录账号的会话。
async fn matching_profile_session(
    state: &AppState,
    normalized_url: &str,
) -> Option<IbbLoginSession> {
    let session = state.login_session.lock().await.clone()?;
    let profile_origin = session.profile.url.trim_end_matches('/');
    if normalized_url.starts_with(profile_origin) {
        return Some(session);
    }

    None
}

/// 读取当前登录会话。
async fn current_login_session(state: &AppState) -> Option<IbbLoginSession> {
    state.login_session.lock().await.clone()
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
            let session = current_login_session(&state).await;
            if session.is_none() && !refresh {
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

            let manager = IbbSpiderManager::new();
            let mut detail = if let Some(session) = session {
                manager
                    .parse_authenticated_album(&session, &normalized_url)
                    .await?
            } else {
                manager.parse_album(&normalized_url).await?
            };
            mark_album_thumbnails_if_enabled(&state, &mut detail, &settings).await?;
            let parsed_at = if current_login_session(&state).await.is_some() {
                chrono::Utc::now().to_rfc3339()
            } else {
                repository::save_album_cache(&state.db_pool, &detail).await?
            };
            emit_album_detail_ready(&window, detail.clone(), false, parsed_at.clone())?;
            if current_login_session(&state).await.is_none() {
                spawn_album_thumbnail_cache(window, &state, detail.clone(), settings);
            }

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
            let session = matching_profile_session(&state, &normalized_url).await;
            if session.is_none() && !refresh {
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

            let manager = IbbSpiderManager::new();
            let authenticated = session.is_some();
            let report: IbbProfileReport = if let Some(session) = session {
                let event_window = window.clone();
                manager
                    .stream_authenticated_profile_albums(
                        &session,
                        &normalized_url,
                        move |batch: IbbProfileBatch| {
                            let event_window = event_window.clone();

                            async move {
                                event_window.emit("profile://album_found", batch)?;
                                Ok(())
                            }
                        },
                    )
                    .await?
            } else {
                let event_window = window.clone();
                manager
                    .stream_profile_albums(&normalized_url, move |batch: IbbProfileBatch| {
                        let event_window = event_window.clone();

                        async move {
                            event_window.emit("profile://album_found", batch)?;
                            Ok(())
                        }
                    })
                    .await?
            };
            let parsed_at = if authenticated {
                chrono::Utc::now().to_rfc3339()
            } else {
                repository::save_profile_cache(&state.db_pool, &normalized_url, &report).await?
            };
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

/// 探测当前聚合搜索是否可用。
#[tauri::command]
pub async fn ping_websearch() -> Result<SearchPing, String> {
    command_result(
        async {
            let search = AggregateSearch::builder().limit(5).build()?;

            search.ping().await
        }
        .await,
    )
}

/// 搜索公开 ImgBB 相册并返回个人空间格式的相册列表。
#[tauri::command]
pub async fn search_imgbb_albums(query: String) -> Result<SearchAlbumsDetail, String> {
    command_result(
        async {
            let query = query.trim().to_string();
            let search_query = build_imgbb_album_search_query(&query);
            let search = AggregateSearch::builder().limit(50).build()?;
            let response = search.search(&search_query).await?;
            let albums = extract_profile_albums_from_search(&response.results);

            Ok(SearchAlbumsDetail {
                query,
                search_query,
                albums,
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

/// 构造限定 ImgBB 域名的相册搜索词。
fn build_imgbb_album_search_query(query: &str) -> String {
    let query = query.trim();
    if query.is_empty() {
        "site:ibb.co".to_string()
    } else {
        format!("site:ibb.co {query}")
    }
}

/// 从搜索结果中提取可识别的 ImgBB 相册列表。
fn extract_profile_albums_from_search(
    results: &[SearchResult],
) -> Vec<imgbb::ibb_spider::IbbProfileAlbum> {
    let mut seen_urls = HashSet::new();
    let mut albums = Vec::new();

    for result in results {
        let mut candidates = collect_album_urls_from_text(&result.url);
        candidates.extend(collect_album_urls_from_text(&result.snippet));

        for url in candidates {
            let Ok(normalized_url) = IbbSpiderManager::normalize_album_url(&url) else {
                continue;
            };
            if !seen_urls.insert(normalized_url.clone()) {
                continue;
            }

            albums.push(imgbb::ibb_spider::IbbProfileAlbum {
                name: clean_search_album_name(&result.title, &normalized_url),
                url: normalized_url,
                cover_url: None,
            });
        }
    }

    albums
}

/// 从文本中收集可能的 ImgBB 相册 URL。
fn collect_album_urls_from_text(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut start_index = 0usize;

    while let Some(relative_index) = text[start_index..].find("ibb.co/album/") {
        let absolute_index = start_index + relative_index;
        let candidate_start = find_url_candidate_start(text, absolute_index);
        let candidate_end = find_url_candidate_end(text, absolute_index);
        let candidate = text[candidate_start..candidate_end]
            .trim_matches(|value: char| matches!(value, '"' | '\'' | ',' | '.' | ')' | ']' | '}'));
        let candidate = if candidate.starts_with("http://") || candidate.starts_with("https://") {
            candidate.to_string()
        } else {
            format!("https://{candidate}")
        };

        urls.push(candidate);
        start_index = candidate_end;
    }

    urls
}

/// 向前定位 URL 候选片段的起始位置。
fn find_url_candidate_start(text: &str, album_marker_index: usize) -> usize {
    let prefix = &text[..album_marker_index];
    let scheme_start = prefix
        .rfind("https://")
        .or_else(|| prefix.rfind("http://"))
        .unwrap_or(album_marker_index);

    if scheme_start + "https://".len() >= album_marker_index
        || scheme_start + "http://".len() >= album_marker_index
    {
        scheme_start
    } else {
        album_marker_index
    }
}

/// 向后定位 URL 候选片段的结束位置。
fn find_url_candidate_end(text: &str, album_marker_index: usize) -> usize {
    text[album_marker_index..]
        .find(|value: char| {
            value.is_whitespace() || matches!(value, '"' | '\'' | '<' | '>' | ')' | ']' | '}')
        })
        .map(|index| album_marker_index + index)
        .unwrap_or(text.len())
}

/// 清理搜索结果标题作为相册展示名称。
fn clean_search_album_name(title: &str, fallback_url: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        return fallback_url.to_string();
    }

    title
        .trim_end_matches("- ImgBB")
        .trim_end_matches("| ImgBB")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证搜索词会限制到 ibb.co 域名。
    #[test]
    fn search_query_adds_site_filter() {
        assert_eq!(
            build_imgbb_album_search_query("demo album"),
            "site:ibb.co demo album"
        );
    }

    /// 验证搜索结果可以提取并规整相册地址。
    #[test]
    fn search_results_extract_album_urls() {
        let results = vec![
            SearchResult {
                title: "Demo - ImgBB".to_string(),
                url: "https://ibb.co/album/ABC123/?sort=name_asc".to_string(),
                snippet: String::new(),
            },
            SearchResult {
                title: "Duplicate".to_string(),
                url: "https://example.com".to_string(),
                snippet: "see https://ibb.co/album/ABC123/ and ibb.co/album/XYZ789".to_string(),
            },
        ];

        let albums = extract_profile_albums_from_search(&results);

        assert_eq!(albums.len(), 2);
        assert_eq!(albums[0].name, "Demo");
        assert_eq!(albums[0].url, "https://ibb.co/album/ABC123/");
        assert_eq!(albums[1].url, "https://ibb.co/album/XYZ789/");
    }
}
