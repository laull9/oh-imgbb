//! favorite 命令负责收藏读写。

use anyhow::Result;
use tauri::State;

use crate::app_state::AppState;
use crate::db::{
    models::{FavoriteInput, FavoriteRecord},
    repository,
};

/// 将 anyhow 错误转换为 Tauri 可序列化错误。
fn command_result<T>(result: Result<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

/// 读取收藏列表。
#[tauri::command]
pub async fn list_favorites(
    state: State<'_, AppState>,
    kind: Option<String>,
) -> Result<Vec<FavoriteRecord>, String> {
    command_result(
        async { repository::list_favorites(&state.db_pool, kind.as_deref()).await }.await,
    )
}

/// 保存收藏。
#[tauri::command]
pub async fn save_favorite(
    state: State<'_, AppState>,
    favorite: FavoriteInput,
) -> Result<(), String> {
    command_result(async { repository::save_favorite(&state.db_pool, &favorite).await }.await)
}

/// 删除收藏。
#[tauri::command]
pub async fn remove_favorite(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    command_result(async { repository::remove_favorite(&state.db_pool, id).await }.await)
}
