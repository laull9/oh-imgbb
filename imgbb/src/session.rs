//! session 模块负责 CLI 登录态的保存、读取和复用。

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use imgbb::ibb_spider::{IbbLoginSession, IbbSpiderManager};

use crate::cli::IbbAuthArgs;

/// CliContext 保存命令执行期间共享的本地上下文。
pub struct CliContext {
    pub session_path: PathBuf,
}

/// 返回可选登录会话。
pub async fn optional_login_session(
    context: &CliContext,
    manager: &IbbSpiderManager,
    auth: &IbbAuthArgs,
) -> Result<Option<IbbLoginSession>> {
    if let Some((login_subject, password)) = resolve_credentials(auth)? {
        let session = manager.login(login_subject, password).await?;
        save_session(&context.session_path, &session)?;
        return Ok(Some(session));
    }

    load_saved_session(&context.session_path)
}

/// 返回必须存在的登录会话。
pub async fn require_login_session(
    context: &CliContext,
    manager: &IbbSpiderManager,
    auth: &IbbAuthArgs,
) -> Result<IbbLoginSession> {
    if let Some((login_subject, password)) = resolve_credentials(auth)? {
        let session = manager.login(login_subject, password).await?;
        save_session(&context.session_path, &session)?;
        return Ok(session);
    }

    if let Some(session) = load_saved_session(&context.session_path)? {
        return Ok(session);
    }

    bail!(
        "此命令需要登录态，请先执行 login，或传入 --login-subject/--password，或设置 IMGBB_LOGIN_SUBJECT/IMGBB_PASSWORD"
    )
}

/// 从 CLI 参数或环境变量读取登录凭据。
pub fn resolve_credentials(auth: &IbbAuthArgs) -> Result<Option<(String, String)>> {
    let login_subject = auth
        .login_subject
        .clone()
        .or_else(|| env::var("IMGBB_LOGIN_SUBJECT").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let password = auth
        .password
        .clone()
        .or_else(|| env::var("IMGBB_PASSWORD").ok())
        .filter(|value| !value.is_empty());

    match (login_subject, password) {
        (Some(login_subject), Some(password)) => Ok(Some((login_subject, password))),
        (None, None) => Ok(None),
        _ => bail!("登录账号和密码必须同时提供"),
    }
}

/// 保存 CLI 登录态到本地文件。
pub fn save_session(path: &Path, session: &IbbLoginSession) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建登录态目录失败: {}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(session)?;
    std::fs::write(path, content).with_context(|| format!("写入登录态失败: {}", path.display()))?;
    set_private_file_permissions(path)?;

    Ok(())
}

/// 读取本地保存的 CLI 登录态。
pub fn load_saved_session(path: &Path) -> Result<Option<IbbLoginSession>> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("读取登录态失败: {}", path.display())),
    };

    serde_json::from_str(&content)
        .with_context(|| format!("解析登录态失败，请重新登录: {}", path.display()))
        .map(Some)
}

/// 在 Unix 平台上限制登录态文件权限。
#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, permissions)
        .with_context(|| format!("设置登录态文件权限失败: {}", path.display()))
}

/// 在非 Unix 平台上跳过权限调整。
#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

/// 解析个人空间 URL，省略时从登录态中读取。
pub fn resolve_profile_url(
    url: Option<String>,
    session: Option<&IbbLoginSession>,
) -> Result<String> {
    if let Some(url) = url {
        return Ok(url);
    }

    let Some(session) = session else {
        bail!("未指定个人空间 URL，也没有可用登录态；请传入 URL 或先执行 login");
    };

    Ok(session.profile.url.clone())
}

/// 输出登录态摘要。
pub fn print_session_summary(session: &IbbLoginSession) {
    println!("登录账号: {}", session.login_subject);
    println!("个人空间: {}", session.profile.url);
    println!("JSON 接口: {}", session.profile.json_url);
    println!("Owner ID: {}", session.profile.owner_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use imgbb::ibb_spider::IbbAuthenticatedProfile;

    /// 创建测试登录态。
    fn sample_session() -> IbbLoginSession {
        IbbLoginSession {
            login_subject: "demo".to_string(),
            redirect_url: "https://ibb.co/auth?login=demo".to_string(),
            profile: IbbAuthenticatedProfile {
                url: "https://demo.imgbb.com/".to_string(),
                json_url: "https://demo.imgbb.com/json".to_string(),
                auth_token: "token".to_string(),
                owner_id: "owner".to_string(),
            },
            cookies: vec![],
            cookie_header: "LID=demo".to_string(),
        }
    }

    /// 验证空登录参数不会触发登录。
    #[test]
    fn credentials_are_optional() {
        let credentials = resolve_credentials(&IbbAuthArgs::default()).unwrap();

        assert!(credentials.is_none());
    }

    /// 验证省略 URL 时可以从登录态读取个人空间。
    #[test]
    fn profile_url_falls_back_to_session() {
        let session = sample_session();

        assert_eq!(
            resolve_profile_url(None, Some(&session)).unwrap(),
            "https://demo.imgbb.com/"
        );
    }

    /// 验证登录态可以保存并读取。
    #[test]
    fn session_round_trips_to_file() {
        let path =
            std::env::temp_dir().join(format!("imgbb-session-test-{}.json", std::process::id()));
        let session = sample_session();

        save_session(&path, &session).unwrap();
        let loaded = load_saved_session(&path).unwrap().unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded, session);
    }
}
