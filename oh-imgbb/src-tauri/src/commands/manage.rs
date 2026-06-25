//! manage 命令负责调用已登录 ImgBB 管理接口。

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use imgbb::ibb_spider::{
    IbbApiReport, IbbCreateAlbumInput, IbbEditImageInput, IbbLoginSession, IbbSpiderManager,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::AppState;

/// UploadAlbumImageInput 保存相册图片上传参数。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UploadAlbumImageInput {
    pub album_id: String,
    pub file_path: String,
}

/// UploadFileInput 保存单文件上传参数。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UploadFileInput {
    pub file_path: String,
}

/// 将 anyhow 错误转换为 Tauri 可序列化错误。
fn command_result<T>(result: Result<T>) -> Result<T, String> {
    result.map_err(|err| err.to_string())
}

/// 读取当前登录会话。
async fn require_login_session(state: &AppState) -> Result<IbbLoginSession> {
    state
        .login_session
        .lock()
        .await
        .clone()
        .ok_or_else(|| anyhow!("请先登录 ImgBB"))
}

/// 创建已登录账号下的新相册。
#[tauri::command]
pub async fn create_imgbb_album(
    state: State<'_, AppState>,
    input: IbbCreateAlbumInput,
) -> Result<IbbApiReport, String> {
    command_result(
        async {
            let session = require_login_session(&state).await?;
            IbbSpiderManager::new().create_album(&session, input).await
        }
        .await,
    )
}

/// 上传图片到指定相册。
#[tauri::command]
pub async fn upload_imgbb_album_image(
    state: State<'_, AppState>,
    input: UploadAlbumImageInput,
) -> Result<IbbApiReport, String> {
    command_result(
        async {
            let session = require_login_session(&state).await?;
            IbbSpiderManager::new()
                .upload_album_image(&session, input.album_id, PathBuf::from(input.file_path))
                .await
        }
        .await,
    )
}

/// 删除指定图片。
#[tauri::command]
pub async fn delete_imgbb_image(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<IbbApiReport, String> {
    command_result(
        async {
            let session = require_login_session(&state).await?;
            IbbSpiderManager::new()
                .delete_image(&session, image_id)
                .await
        }
        .await,
    )
}

/// 删除指定相册。
#[tauri::command]
pub async fn delete_imgbb_album(
    state: State<'_, AppState>,
    album_id: String,
) -> Result<IbbApiReport, String> {
    command_result(
        async {
            let session = require_login_session(&state).await?;
            IbbSpiderManager::new()
                .delete_album(&session, album_id)
                .await
        }
        .await,
    )
}

/// 上传个人主页背景图。
#[tauri::command]
pub async fn upload_imgbb_profile_background(
    state: State<'_, AppState>,
    input: UploadFileInput,
) -> Result<IbbApiReport, String> {
    command_result(
        async {
            let session = require_login_session(&state).await?;
            IbbSpiderManager::new()
                .upload_profile_background(&session, PathBuf::from(input.file_path))
                .await
        }
        .await,
    )
}

/// 删除个人主页背景图。
#[tauri::command]
pub async fn delete_imgbb_profile_background(
    state: State<'_, AppState>,
) -> Result<IbbApiReport, String> {
    command_result(
        async {
            let session = require_login_session(&state).await?;
            IbbSpiderManager::new()
                .delete_profile_background(&session)
                .await
        }
        .await,
    )
}

/// 编辑图片标题、描述或所属相册。
#[tauri::command]
pub async fn edit_imgbb_image(
    state: State<'_, AppState>,
    input: IbbEditImageInput,
) -> Result<IbbApiReport, String> {
    command_result(
        async {
            let session = require_login_session(&state).await?;
            IbbSpiderManager::new().edit_image(&session, input).await
        }
        .await,
    )
}
