//! download 命令负责创建和管理 ImgBB 下载任务。

use anyhow::{bail, Result};
use imgbb::ibb_spider::{
    IbbAlbumDetail, IbbDownloadProgressEvent, IbbLoginSession, IbbSpiderManager,
};
use llpha::{LlphaClient, RetryPolicy};
use tauri::{Emitter, State, Window};

use crate::app_state::AppState;
use crate::db::{models::DownloadReport, repository};
use crate::download_tasks::{DownloadTaskRecord, DownloadTaskStatus, DownloadTaskStore};
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
    let client = LlphaClient::builder()
        .max_concurrent_requests(settings.max_concurrent_downloads)
        .retry_policy(RetryPolicy::new(settings.max_retries))
        .build()?;
    let mut manager = IbbSpiderManager::new()
        .with_client(std::sync::Arc::new(client))
        .with_base_path(&settings.download_dir);

    if let Some(pattern) = settings.file_name_pattern.filter(|value| !value.is_empty()) {
        manager = manager.with_file_name_pattern(pattern);
    }

    Ok(manager)
}

/// 读取当前登录会话。
async fn current_login_session(state: &AppState) -> Option<IbbLoginSession> {
    state.login_session.lock().await.clone()
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

/// 推送下载任务更新事件。
fn emit_task_update(window: &Window, task: &DownloadTaskRecord) {
    if let Err(err) = window.emit("download://task_updated", task) {
        tracing::warn!(task_id = task.id, error = %err, "推送下载任务事件失败");
    }
}

/// 推送可选任务更新事件。
fn emit_optional_task_update(window: &Window, task: Option<DownloadTaskRecord>) {
    if let Some(task) = task {
        emit_task_update(window, &task);
    }
}

/// 创建下载进度回调。
fn progress_callback(
    window: Window,
    store: std::sync::Arc<DownloadTaskStore>,
    task_id: u64,
) -> impl Fn(
    imgbb::ibb_spider::IbbDownloadProgress,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
       + Send
       + Sync
       + 'static {
    move |progress| {
        let window = window.clone();
        let store = store.clone();

        Box::pin(async move {
            let task = match progress.event {
                IbbDownloadProgressEvent::TotalKnown => {
                    store
                        .update_title_and_add_total_items(
                            task_id,
                            progress.album_title,
                            progress.total_files,
                        )
                        .await
                }
                IbbDownloadProgressEvent::FileFinished => {
                    store
                        .add_finished_item(task_id, progress.bytes_written)
                        .await
                }
            };
            emit_optional_task_update(&window, task);
        })
    }
}

/// 运行单相册 URL 下载任务。
async fn run_album_url_task(
    window: Window,
    store: std::sync::Arc<DownloadTaskStore>,
    task_id: u64,
    manager: IbbSpiderManager,
    url: String,
    session: Option<IbbLoginSession>,
) {
    emit_optional_task_update(&window, store.mark_running(task_id).await);

    let result = async {
        let progress = progress_callback(window.clone(), store.clone(), task_id);
        let report = if let Some(session) = session.as_ref() {
            manager
                .download_authenticated_album_with_progress(session, url, progress)
                .await?
        } else {
            manager.download_album_with_progress(url, progress).await?
        };
        let report = to_download_report(report);
        Ok::<_, anyhow::Error>((report.downloaded_files, report.bytes_written))
    }
    .await;

    finish_task(window, store, task_id, result).await;
}

/// 运行已解析相册的选中图片下载任务。
async fn run_album_images_task(
    window: Window,
    store: std::sync::Arc<DownloadTaskStore>,
    task_id: u64,
    manager: IbbSpiderManager,
    album: IbbAlbumDetail,
    image_ids: Vec<String>,
    session: Option<IbbLoginSession>,
) {
    emit_optional_task_update(&window, store.mark_running(task_id).await);

    let result = async {
        let progress = progress_callback(window.clone(), store.clone(), task_id);
        let report = if let Some(session) = session.as_ref() {
            manager
                .download_authenticated_album_images_with_progress(
                    session, &album, &image_ids, progress,
                )
                .await?
        } else {
            manager
                .download_album_images_with_progress(&album, &image_ids, progress)
                .await?
        };
        let report = to_download_report(report);
        Ok::<_, anyhow::Error>((report.downloaded_files, report.bytes_written))
    }
    .await;

    finish_task(window, store, task_id, result).await;
}

/// 运行个人空间选中相册批量下载任务。
async fn run_profile_batch_task(
    window: Window,
    store: std::sync::Arc<DownloadTaskStore>,
    task_id: u64,
    manager: IbbSpiderManager,
    urls: Vec<String>,
    session: Option<IbbLoginSession>,
    cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    emit_optional_task_update(&window, store.mark_running(task_id).await);

    for url in urls {
        if DownloadTaskStore::is_cancelled(&cancel_flag) {
            emit_optional_task_update(&window, store.mark_cancelled(task_id).await);
            return;
        }

        let progress = progress_callback(window.clone(), store.clone(), task_id);
        let result = if let Some(session) = session.as_ref() {
            manager
                .download_authenticated_album_with_progress(session, url, progress)
                .await
        } else {
            manager.download_album_with_progress(url, progress).await
        }
        .map(to_download_report);
        match result {
            Ok(_report) => {}
            Err(err) => {
                emit_optional_task_update(
                    &window,
                    store.mark_failed(task_id, err.to_string()).await,
                );
                return;
            }
        }
    }

    emit_optional_task_update(&window, store.mark_completed(task_id).await);
}

/// 按下载结果完成单相册任务。
async fn finish_task(
    window: Window,
    store: std::sync::Arc<DownloadTaskStore>,
    task_id: u64,
    result: Result<(usize, usize)>,
) {
    match result {
        Ok((_downloaded_files, _bytes_written)) => {
            emit_optional_task_update(&window, store.mark_completed(task_id).await);
        }
        Err(err) => {
            emit_optional_task_update(&window, store.mark_failed(task_id, err.to_string()).await);
        }
    }
}

/// 下载整本相册，返回后台任务记录。
#[tauri::command]
pub async fn download_album(
    window: Window,
    state: State<'_, AppState>,
    url: String,
) -> Result<DownloadTaskRecord, String> {
    command_result(
        async {
            let normalized_url = IbbSpiderManager::normalize_album_url(&url)?;
            let manager = configured_manager(&state).await?;
            let session = current_login_session(&state).await;
            let store = state.download_tasks.clone();
            let (task, _) = store
                .create_task(
                    "相册下载".to_string(),
                    "album".to_string(),
                    normalized_url.clone(),
                    0,
                )
                .await;
            let task_id = task.id;
            let handle = tauri::async_runtime::spawn(run_album_url_task(
                window,
                store.clone(),
                task_id,
                manager,
                normalized_url,
                session,
            ));
            store.attach_handle(task_id, handle).await;

            Ok(task)
        }
        .await,
    )
}

/// 下载相册中选中的图片，返回后台任务记录。
#[tauri::command]
pub async fn download_album_images(
    window: Window,
    state: State<'_, AppState>,
    album: IbbAlbumDetail,
    image_ids: Vec<String>,
) -> Result<DownloadTaskRecord, String> {
    command_result(
        async {
            if image_ids.is_empty() {
                bail!("请选择要下载的图片");
            }

            let manager = configured_manager(&state).await?;
            let session = current_login_session(&state).await;
            let store = state.download_tasks.clone();
            let (task, _) = store
                .create_task(
                    album.title.clone(),
                    "album".to_string(),
                    album.url.clone(),
                    0,
                )
                .await;
            let task_id = task.id;
            let handle = tauri::async_runtime::spawn(run_album_images_task(
                window,
                store.clone(),
                task_id,
                manager,
                album,
                image_ids,
                session,
            ));
            store.attach_handle(task_id, handle).await;

            Ok(task)
        }
        .await,
    )
}

/// 批量下载选中的个人空间相册，返回后台任务记录。
#[tauri::command]
pub async fn download_profile_albums(
    window: Window,
    state: State<'_, AppState>,
    urls: Vec<String>,
) -> Result<DownloadTaskRecord, String> {
    command_result(
        async {
            if urls.is_empty() {
                bail!("请选择要下载的相册");
            }

            let manager = configured_manager(&state).await?;
            let session = current_login_session(&state).await;
            let store = state.download_tasks.clone();
            let title = format!("批量下载 {} 个相册", urls.len());
            let target_url = urls.first().cloned().unwrap_or_default();
            let (task, cancel_flag) = store
                .create_task(title, "profile".to_string(), target_url, 0)
                .await;
            let task_id = task.id;
            let handle = tauri::async_runtime::spawn(run_profile_batch_task(
                window,
                store.clone(),
                task_id,
                manager,
                urls,
                session,
                cancel_flag,
            ));
            store.attach_handle(task_id, handle).await;

            Ok(task)
        }
        .await,
    )
}

/// 读取下载任务列表。
#[tauri::command]
pub async fn list_download_tasks(
    state: State<'_, AppState>,
) -> Result<Vec<DownloadTaskRecord>, String> {
    command_result(async { Ok(state.download_tasks.list_tasks().await) }.await)
}

/// 取消指定下载任务。
#[tauri::command]
pub async fn cancel_download_task(
    window: Window,
    state: State<'_, AppState>,
    id: u64,
) -> Result<DownloadTaskRecord, String> {
    command_result(
        async {
            let task = state
                .download_tasks
                .cancel_task(id)
                .await
                .ok_or_else(|| anyhow::anyhow!("下载任务不存在: {id}"))?;
            if task.status == DownloadTaskStatus::Cancelled {
                emit_task_update(&window, &task);
            }

            Ok(task)
        }
        .await,
    )
}
