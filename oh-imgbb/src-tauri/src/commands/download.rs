//! download 命令负责启动 ImgBB 下载任务。

use anyhow::Result;
use imgbb::ibb_spider::IbbSpiderManager;
use tauri::State;

use crate::app_state::AppState;
use crate::db::{models::DownloadReport, repository};
use crate::settings::AppSettings;

/// 将 anyhow 错误转换为 Tauri 可序列化错误。
fn command_result<T>(result: Result<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

/// 下载整本相册。
#[tauri::command]
pub async fn download_album(
    state: State<'_, AppState>,
    url: String,
) -> Result<DownloadReport, String> {
    command_result(
        async {
            let settings = repository::load_settings(&state.db_pool)
                .await?
                .unwrap_or_else(|| AppSettings::with_download_dir(&state.default_download_dir));
            let mut manager = IbbSpiderManager::new().with_base_path(&settings.download_dir);

            if let Some(pattern) = settings.file_name_pattern.filter(|value| !value.is_empty()) {
                manager = manager.with_file_name_pattern(pattern);
            }

            let report = manager.download_album(url).await?;

            Ok(DownloadReport {
                normalized_url: report.normalized_url,
                author_url: report.author_url,
                directory: report.download_summary.directory.display().to_string(),
                downloaded_files: report.download_summary.downloaded_files,
                bytes_written: report.download_summary.bytes_written,
            })
        }
        .await,
    )
}
