//! app_state 模块保存 Tauri 后端共享状态。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use imgbb::ibb_spider::IbbLoginSession;
use llpha::LlphaClient;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::db;
use crate::download_tasks::DownloadTaskStore;
use crate::settings::AppSettings;

const THUMBNAIL_CACHE_CONCURRENCY: usize = 32;

/// AppState 保存数据库连接池和应用目录。
pub struct AppState {
    pub db_pool: SqlitePool,
    pub default_download_dir: PathBuf,
    pub thumbnail_cache_dir: PathBuf,
    pub detail_image_dir: PathBuf,
    pub thumbnail_client: Arc<LlphaClient>,
    pub download_tasks: Arc<DownloadTaskStore>,
    pub login_session: Arc<Mutex<Option<IbbLoginSession>>>,
}

impl AppState {
    /// 初始化应用目录、数据库和默认配置。
    pub async fn initialize(app: &AppHandle) -> Result<Self> {
        let app_dir = resolve_app_dir()?;
        tokio::fs::create_dir_all(&app_dir).await?;
        let db_path = app_dir.join("oh-imgbb.sqlite3");
        let db_pool = db::connect(&db_path).await?;
        db::init_schema(&db_pool).await?;

        let default_download_dir = resolve_default_download_dir(app, &app_dir)?;
        let thumbnail_cache_dir = resolve_thumbnail_cache_dir(&app_dir);
        let detail_image_dir = resolve_detail_image_dir(&app_dir);
        tokio::fs::create_dir_all(&thumbnail_cache_dir).await?;
        tokio::fs::create_dir_all(&detail_image_dir).await?;
        let thumbnail_client = Arc::new(
            LlphaClient::builder()
                .max_concurrent_requests(THUMBNAIL_CACHE_CONCURRENCY)
                .build()?,
        );
        let download_tasks = Arc::new(DownloadTaskStore::new());
        let login_session = Arc::new(Mutex::new(None));
        let default_settings = AppSettings::with_download_dir(&default_download_dir);
        db::repository::ensure_settings(&db_pool, &default_settings).await?;

        Ok(Self {
            db_pool,
            default_download_dir,
            thumbnail_cache_dir,
            detail_image_dir,
            thumbnail_client,
            download_tasks,
            login_session,
        })
    }
}

/// 解析程序所在目录作为 SQLite 默认目录。
fn resolve_app_dir() -> Result<PathBuf> {
    let current_exe = std::env::current_exe()?;
    if let Some(parent) = current_exe.parent() {
        return Ok(parent.to_path_buf());
    }

    Ok(std::env::current_dir()?)
}

/// 解析系统下载目录作为默认下载目录。
fn resolve_default_download_dir(app: &AppHandle, fallback_dir: &Path) -> Result<PathBuf> {
    match app.path().download_dir() {
        Ok(download_dir) => Ok(download_dir),
        Err(_) => Ok(fallback_dir.to_path_buf()),
    }
}

/// 解析程序目录下的缩略图缓存目录。
fn resolve_thumbnail_cache_dir(app_dir: &std::path::Path) -> PathBuf {
    app_dir.join("small-imgs")
}

/// 解析程序目录下的详情图临时目录。
fn resolve_detail_image_dir(app_dir: &std::path::Path) -> PathBuf {
    app_dir.join("detail-tmp")
}
