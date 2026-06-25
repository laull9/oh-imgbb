//! auth 命令负责管理 ImgBB 登录态。

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use imgbb::ibb_spider::{IbbLoginSession, IbbSpiderManager};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use tokio::sync::Mutex;

use crate::app_state::AppState;
use crate::db::repository;
use crate::settings::AppSettings;

/// LoginStatus 保存前端可见的登录状态。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LoginStatus {
    pub logged_in: bool,
    pub verified: bool,
    pub login_subject: Option<String>,
    pub redirect_url: Option<String>,
    pub profile_url: Option<String>,
    pub json_url: Option<String>,
    pub owner_id: Option<String>,
}

impl LoginStatus {
    /// 根据后端会话创建可展示状态。
    fn from_session(session: Option<&IbbLoginSession>) -> Self {
        match session {
            Some(session) => Self {
                logged_in: true,
                verified: true,
                login_subject: Some(session.login_subject.clone()),
                redirect_url: Some(session.redirect_url.clone()),
                profile_url: Some(session.profile.url.clone()),
                json_url: Some(session.profile.json_url.clone()),
                owner_id: Some(session.profile.owner_id.clone()),
            },
            None => Self {
                logged_in: false,
                verified: false,
                login_subject: None,
                redirect_url: None,
                profile_url: None,
                json_url: None,
                owner_id: None,
            },
        }
    }
}

/// 将 anyhow 错误转换为 Tauri 可序列化错误。
fn command_result<T>(result: Result<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

/// 读取当前设置或默认设置。
async fn load_settings_or_default(state: &AppState) -> Result<AppSettings> {
    Ok(repository::load_settings(&state.db_pool)
        .await?
        .unwrap_or_else(|| AppSettings::with_download_dir(&state.default_download_dir)))
}

/// 保存登录凭据到应用设置。
async fn save_login_credentials(
    state: &AppState,
    login_subject: &str,
    password: &str,
) -> Result<()> {
    let mut settings = load_settings_or_default(state).await?;
    settings.imgbb_login_subject = Some(login_subject.trim().to_string());
    settings.imgbb_password = Some(password.to_string());
    repository::save_settings(&state.db_pool, &settings).await
}

/// 清理设置中的登录凭据。
async fn clear_login_credentials(state: &AppState) -> Result<()> {
    let mut settings = load_settings_or_default(state).await?;
    settings.imgbb_login_subject = None;
    settings.imgbb_password = None;
    repository::save_settings(&state.db_pool, &settings).await
}

/// 尝试使用设置中保存的凭据恢复登录态。
async fn login_from_saved_settings(
    db_pool: SqlitePool,
    default_download_dir: PathBuf,
    login_session: Arc<Mutex<Option<IbbLoginSession>>>,
) -> Result<()> {
    let settings = repository::load_settings(&db_pool)
        .await?
        .unwrap_or_else(|| AppSettings::with_download_dir(&default_download_dir));
    let Some(login_subject) = settings
        .imgbb_login_subject
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return Ok(());
    };
    let Some(password) = settings.imgbb_password.filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let session = IbbSpiderManager::new()
        .login(&login_subject, &password)
        .await?;
    *login_session.lock().await = Some(session);

    Ok(())
}

/// 启动后台任务，在应用打开后立即尝试恢复登录。
pub fn spawn_saved_login(state: &AppState) {
    let db_pool = state.db_pool.clone();
    let default_download_dir = state.default_download_dir.clone();
    let login_session = state.login_session.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(err) =
            login_from_saved_settings(db_pool, default_download_dir, login_session).await
        {
            tracing::warn!(error = %err, "恢复 ImgBB 登录失败");
        }
    });
}

/// 登录 ImgBB 并把 Cookie 会话保存在内存中。
#[tauri::command]
pub async fn login_imgbb(
    state: State<'_, AppState>,
    login_subject: String,
    password: String,
) -> Result<LoginStatus, String> {
    command_result(
        async {
            let session = IbbSpiderManager::new()
                .login(&login_subject, &password)
                .await?;
            let status = LoginStatus::from_session(Some(&session));
            save_login_credentials(&state, &login_subject, &password).await?;
            *state.login_session.lock().await = Some(session);

            Ok(status)
        }
        .await,
    )
}

/// 读取当前 ImgBB 登录状态。
#[tauri::command]
pub async fn get_imgbb_login_status(state: State<'_, AppState>) -> Result<LoginStatus, String> {
    let session = state.login_session.lock().await;

    Ok(LoginStatus::from_session(session.as_ref()))
}

/// 清空当前 ImgBB 登录状态。
#[tauri::command]
pub async fn logout_imgbb(state: State<'_, AppState>) -> Result<LoginStatus, String> {
    command_result(
        async {
            *state.login_session.lock().await = None;
            clear_login_credentials(&state).await?;

            Ok(LoginStatus::from_session(None))
        }
        .await,
    )
}
