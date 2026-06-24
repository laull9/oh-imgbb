//! app_state 模块保存 Tauri 后端共享状态。

use std::path::PathBuf;

use anyhow::Result;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::db;
use crate::settings::AppSettings;

/// AppState 保存数据库连接池和应用目录。
pub struct AppState {
    pub db_pool: SqlitePool,
    pub default_download_dir: PathBuf,
}

impl AppState {
    /// 初始化应用目录、数据库和默认配置。
    pub async fn initialize(app: &AppHandle) -> Result<Self> {
        let app_dir = resolve_app_dir(app)?;
        tokio::fs::create_dir_all(&app_dir).await?;
        let db_path = app_dir.join("oh-imgbb.sqlite3");
        let db_pool = db::connect(&db_path).await?;
        db::init_schema(&db_pool).await?;

        let default_download_dir = resolve_default_download_dir(app, &app_dir)?;
        let default_settings = AppSettings::with_download_dir(&default_download_dir);
        db::repository::ensure_settings(&db_pool, &default_settings).await?;

        Ok(Self {
            db_pool,
            default_download_dir,
        })
    }
}

/// 解析程序所在目录作为 SQLite 默认目录。
fn resolve_app_dir(app: &AppHandle) -> Result<PathBuf> {
    let current_exe = std::env::current_exe()?;
    if let Some(parent) = current_exe.parent() {
        return Ok(parent.to_path_buf());
    }

    Ok(app.path().app_data_dir()?)
}

/// 解析系统文档目录作为默认下载目录。
fn resolve_default_download_dir(app: &AppHandle, fallback_dir: &PathBuf) -> Result<PathBuf> {
    match app.path().document_dir() {
        Ok(document_dir) => Ok(document_dir),
        Err(_) => Ok(fallback_dir.to_path_buf()),
    }
}
