//! download 命令负责启动 ImgBB 下载任务。

use anyhow::Result;
use imgbb::ibb_spider::{IbbAlbumDetail, IbbSpiderManager};
use tauri::State;

use crate::app_state::AppState;
use crate::db::{
    models::{DownloadBatchReport, DownloadReport},
    repository,
};
use crate::settings::AppSettings;

/// 将 anyhow 错误转换为 Tauri 可序列化错误。
fn command_result<T>(result: Result<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

/// 创建按应用设置配置好的下载管理器。
async fn configured_manager(state: &AppState) -> Result<IbbSpiderManager> {
    let settings = repository::load_settings(&state.db_pool)
        .await?
        .unwrap_or_else(|| AppSettings::with_download_dir(&state.default_download_dir));
    let mut manager = IbbSpiderManager::new().with_base_path(&settings.download_dir);

    if let Some(pattern) = settings.file_name_pattern.filter(|value| !value.is_empty()) {
        manager = manager.with_file_name_pattern(pattern);
    }

    Ok(manager)
}

/// 转换下载摘要为前端结构。
fn to_download_report(report: imgbb::ibb_spider::IbbSpiderReport) -> DownloadReport {
    DownloadReport {
        normalized_url: report.normalized_url,
        author_url: report.author_url,
        directory: report.download_summary.directory.display().to_string(),
        downloaded_files: report.download_summary.downloaded_files,
        bytes_written: report.download_summary.bytes_written,
    }
}

/// 下载整本相册。
#[tauri::command]
pub async fn download_album(
    state: State<'_, AppState>,
    url: String,
) -> Result<DownloadReport, String> {
    command_result(
        async {
            let manager = configured_manager(&state).await?;
            let report = manager.download_album(url).await?;

            Ok(to_download_report(report))
        }
        .await,
    )
}

/// 下载相册中选中的图片。
#[tauri::command]
pub async fn download_album_images(
    state: State<'_, AppState>,
    album: IbbAlbumDetail,
    image_ids: Vec<String>,
) -> Result<DownloadReport, String> {
    command_result(
        async {
            let manager = configured_manager(&state).await?;
            let report = manager.download_album_images(&album, &image_ids).await?;

            Ok(to_download_report(report))
        }
        .await,
    )
}

/// 批量下载选中的个人空间相册。
#[tauri::command]
pub async fn download_profile_albums(
    state: State<'_, AppState>,
    urls: Vec<String>,
) -> Result<DownloadBatchReport, String> {
    command_result(
        async {
            let manager = configured_manager(&state).await?;
            let mut reports = Vec::with_capacity(urls.len());

            for url in urls {
                let report = manager.download_album(url).await?;
                reports.push(to_download_report(report));
            }

            let downloaded_files = reports
                .iter()
                .map(|report| report.downloaded_files)
                .sum::<usize>();
            let bytes_written = reports
                .iter()
                .map(|report| report.bytes_written)
                .sum::<usize>();

            Ok(DownloadBatchReport {
                reports,
                downloaded_files,
                bytes_written,
            })
        }
        .await,
    )
}
