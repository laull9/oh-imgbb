//! settings 命令负责读取和更新应用设置。

use anyhow::Result;
use tauri::State;

use crate::app_state::AppState;
use crate::db::repository;
use crate::settings::AppSettings;

/// 将 anyhow 错误转换为 Tauri 可序列化错误。
fn command_result<T>(result: Result<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

/// 读取应用设置。
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    command_result(
        async {
            Ok(repository::load_settings(&state.db_pool)
                .await?
                .unwrap_or_else(|| AppSettings::with_download_dir(&state.default_download_dir)))
        }
        .await,
    )
}

/// 更新应用设置。
#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    command_result(
        async {
            repository::save_settings(&state.db_pool, &settings).await?;
            Ok(settings)
        }
        .await,
    )
}
