use super::login::IbbLoginSession;
use super::manage_response::{find_id, find_image_id, find_url};
use super::utils::extract_auth_token;
use anyhow::{Context, Result, ensure};
use reqwest::{
    Client, Url,
    header::{
        ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, COOKIE, HeaderMap, HeaderValue, ORIGIN, REFERER,
        USER_AGENT,
    },
    multipart,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
const IBB_JSON_URL: &str = "https://ibb.co/json";
const DEFAULT_ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9";
const DEFAULT_BROWSER_USER_AGENT: &str = llpha::DEFAULT_BROWSER_USER_AGENT;
const FORM_ACCEPT: &str = "application/json, text/javascript, */*; q=0.01";
const UPLOAD_ACCEPT: &str = "application/json";
/// IbbAlbumPrivacy 表示创建相册时的可见性。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IbbAlbumPrivacy {
    Public,
    Private,
    Password,
}

impl IbbAlbumPrivacy {
    /// 返回 ImgBB 表单使用的隐私字段。
    fn as_form_value(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Password => "password",
        }
    }
}

/// IbbCreateAlbumInput 保存创建相册参数。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IbbCreateAlbumInput {
    pub name: String,
    pub description: Option<String>,
    pub privacy: IbbAlbumPrivacy,
    pub password: Option<String>,
}

/// IbbEditImageInput 保存编辑图片信息参数。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IbbEditImageInput {
    pub image_id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub album_id: Option<String>,
    pub new_album: bool,
}

/// IbbApiReport 保存 ImgBB 管理接口响应摘要。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IbbApiReport {
    pub status_code: u64,
    pub id: Option<String>,
    pub url: Option<String>,
    pub raw: Value,
}

/// 创建已登录账号下的新相册。
pub(super) async fn create_album(
    session: &IbbLoginSession,
    input: IbbCreateAlbumInput,
) -> Result<IbbApiReport> {
    ensure!(!input.name.trim().is_empty(), "相册名称不能为空");
    if input.privacy == IbbAlbumPrivacy::Password {
        ensure!(
            input
                .password
                .as_deref()
                .map(str::trim)
                .is_some_and(|password| !password.is_empty()),
            "密码相册必须填写密码"
        );
    }

    let mut fields = vec![
        ("auth_token".to_string(), session.profile.auth_token.clone()),
        ("pathname".to_string(), "/".to_string()),
        ("action".to_string(), "create-album".to_string()),
        ("type".to_string(), "album".to_string()),
        ("album[name]".to_string(), input.name.trim().to_string()),
        (
            "album[description]".to_string(),
            input.description.unwrap_or_default(),
        ),
        (
            "album[privacy]".to_string(),
            input.privacy.as_form_value().to_string(),
        ),
        ("album[new]".to_string(), "true".to_string()),
    ];
    if let Some(password) = input.password {
        fields.push(("album[password]".to_string(), password));
    }

    post_profile_form(session, fields).await
}

/// 上传文件到指定相册。
pub(super) async fn upload_album_image(
    session: &IbbLoginSession,
    album_id: &str,
    file_path: &Path,
) -> Result<IbbApiReport> {
    ensure!(!album_id.trim().is_empty(), "相册 ID 不能为空");
    let album_url = format!("https://ibb.co/album/{}", album_id.trim());
    let auth_token = fetch_album_auth_token(session, &album_url).await?;
    let bytes = fs::read(file_path)
        .await
        .with_context(|| format!("读取上传文件失败: {}", file_path.display()))?;
    ensure!(!bytes.is_empty(), "上传文件不能为空");
    let file_name = file_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("upload.bin")
        .to_string();
    let mime = mime_from_path(file_path);
    let form = multipart::Form::new()
        .text("type", "file")
        .text("action", "upload")
        .text("timestamp", current_millis().to_string())
        .text("auth_token", auth_token)
        .text("owner", session.profile.owner_id.clone())
        .text("album", album_id.trim().to_string())
        .text("album_id", album_id.trim().to_string())
        .part("source", multipart_part(file_name, bytes, mime)?);

    let report = post_multipart(session, IBB_JSON_URL, &album_url, form).await?;
    if let Some(image_id) = find_image_id(&report.raw) {
        edit_image(
            session,
            IbbEditImageInput {
                image_id,
                title: None,
                description: None,
                album_id: Some(album_id.trim().to_string()),
                new_album: false,
            },
        )
        .await?;
    }

    Ok(report)
}

/// 删除指定图片。
pub(super) async fn delete_image(
    session: &IbbLoginSession,
    image_id: &str,
) -> Result<IbbApiReport> {
    let image_id = normalize_image_management_id(image_id);
    ensure!(!image_id.is_empty(), "图片 ID 不能为空");

    post_profile_form(
        session,
        vec![
            ("auth_token".to_string(), session.profile.auth_token.clone()),
            ("pathname".to_string(), "/".to_string()),
            ("action".to_string(), "delete".to_string()),
            ("single".to_string(), "true".to_string()),
            ("delete".to_string(), "image".to_string()),
            ("deleting[id]".to_string(), image_id),
        ],
    )
    .await
}

/// 删除指定相册。
pub(super) async fn delete_album(
    session: &IbbLoginSession,
    album_id: &str,
) -> Result<IbbApiReport> {
    ensure!(!album_id.trim().is_empty(), "相册 ID 不能为空");

    post_profile_form(
        session,
        vec![
            ("auth_token".to_string(), session.profile.auth_token.clone()),
            ("pathname".to_string(), "/".to_string()),
            ("action".to_string(), "delete".to_string()),
            ("single".to_string(), "true".to_string()),
            ("delete".to_string(), "album".to_string()),
            ("deleting[id]".to_string(), album_id.trim().to_string()),
        ],
    )
    .await
}

/// 上传个人主页背景图。
pub(super) async fn upload_profile_background(
    session: &IbbLoginSession,
    file_path: &Path,
) -> Result<IbbApiReport> {
    let bytes = fs::read(file_path)
        .await
        .with_context(|| format!("读取背景图片失败: {}", file_path.display()))?;
    ensure!(!bytes.is_empty(), "背景图片不能为空");
    let file_name = file_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("background.bin")
        .to_string();
    let mime = mime_from_path(file_path);
    let form = multipart::Form::new()
        .text("action", "upload")
        .text("type", "file")
        .text("what", "background")
        .text("owner", session.profile.owner_id.clone())
        .text("auth_token", session.profile.auth_token.clone())
        .part("source", multipart_part(file_name, bytes, mime)?);

    post_multipart(
        session,
        &session.profile.json_url,
        &session.profile.url,
        form,
    )
    .await
}

/// 删除个人主页背景图。
pub(super) async fn delete_profile_background(session: &IbbLoginSession) -> Result<IbbApiReport> {
    post_profile_form(
        session,
        vec![
            ("auth_token".to_string(), session.profile.auth_token.clone()),
            ("pathname".to_string(), "/".to_string()),
            ("action".to_string(), "delete".to_string()),
            ("delete".to_string(), "background".to_string()),
            ("owner".to_string(), session.profile.owner_id.clone()),
        ],
    )
    .await
}

/// 编辑图片标题、描述或所属相册。
pub(super) async fn edit_image(
    session: &IbbLoginSession,
    input: IbbEditImageInput,
) -> Result<IbbApiReport> {
    let image_id = normalize_image_management_id(&input.image_id);
    ensure!(!image_id.is_empty(), "图片 ID 不能为空");
    let mut fields = vec![
        ("auth_token".to_string(), session.profile.auth_token.clone()),
        ("pathname".to_string(), "/".to_string()),
        ("action".to_string(), "edit".to_string()),
        ("edit".to_string(), "image".to_string()),
        ("single".to_string(), "true".to_string()),
        ("owner".to_string(), session.profile.owner_id.clone()),
        ("editing[id]".to_string(), image_id),
        (
            "editing[description]".to_string(),
            input.description.unwrap_or_default(),
        ),
        (
            "editing[title]".to_string(),
            input.title.unwrap_or_default(),
        ),
        (
            "editing[new_album]".to_string(),
            input.new_album.to_string(),
        ),
    ];
    if let Some(album_id) = input.album_id {
        fields.push(("editing[album_id]".to_string(), album_id));
    }

    post_profile_form(session, fields).await
}

/// 规整管理接口使用的图片 ID。
fn normalize_image_management_id(input: &str) -> String {
    let input = input.trim();
    image_id_from_url(input).unwrap_or_else(|| input.to_string())
}

/// 从图片或查看页 URL 中提取 ImgBB encoded ID。
fn image_id_from_url(input: &str) -> Option<String> {
    let url = Url::parse(input).ok()?;
    let host = url.host_str()?;
    if !host.ends_with("ibb.co") {
        return None;
    }

    url.path_segments()?
        .find(|segment| !segment.is_empty() && *segment != "album")
        .map(str::to_string)
}

/// 拉取相册页面并提取上传使用的 token。
async fn fetch_album_auth_token(session: &IbbLoginSession, album_url: &str) -> Result<String> {
    let response = http_client()
        .get(album_url)
        .headers(page_headers(session, &session.profile.url)?)
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "ImgBB 相册页请求失败: {} {}",
        response.status(),
        album_url
    );
    let html = response.text().await?;

    extract_auth_token(&html)
}

/// 发送用户子域表单接口。
async fn post_profile_form(
    session: &IbbLoginSession,
    fields: Vec<(String, String)>,
) -> Result<IbbApiReport> {
    let response = http_client()
        .post(&session.profile.json_url)
        .headers(form_headers(
            session,
            &session.profile.url,
            FORM_ACCEPT,
            true,
        )?)
        .body(build_form_body(fields)?)
        .send()
        .await?;

    parse_json_response(response).await
}

/// 发送 multipart 上传接口。
async fn post_multipart(
    session: &IbbLoginSession,
    url: &str,
    referer: &str,
    form: multipart::Form,
) -> Result<IbbApiReport> {
    let response = http_client()
        .post(url)
        .headers(form_headers(session, referer, UPLOAD_ACCEPT, false)?)
        .multipart(form)
        .send()
        .await?;

    parse_json_response(response).await
}

/// 解析 ImgBB JSON 管理接口响应。
async fn parse_json_response(response: reqwest::Response) -> Result<IbbApiReport> {
    let status = response.status();
    let url = response.url().to_string();
    let body = response.text().await?;
    ensure!(
        status.is_success(),
        "ImgBB 管理接口请求失败: {status} {url} {body}"
    );
    let raw: Value = serde_json::from_str(&body).context("解析 ImgBB 管理接口 JSON 失败")?;
    let status_code = raw
        .get("status_code")
        .and_then(Value::as_u64)
        .unwrap_or(status.as_u16() as u64);
    ensure!(status_code == 200, "ImgBB 管理接口返回异常: {}", raw);

    Ok(IbbApiReport {
        status_code,
        id: find_id(&raw),
        url: find_url(&raw),
        raw,
    })
}

/// 构造 multipart 文件字段。
fn multipart_part(
    file_name: String,
    bytes: Vec<u8>,
    mime: &'static str,
) -> Result<multipart::Part> {
    multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(mime)
        .map_err(Into::into)
}

/// 根据文件扩展名推断常见图片 MIME。
fn mime_from_path(file_path: &Path) -> &'static str {
    match file_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

/// 构造普通页面请求头。
fn page_headers(session: &IbbLoginSession, referer: &str) -> Result<HeaderMap> {
    let mut headers = base_headers(session)?;
    insert_header(&mut headers, REFERER, referer)?;
    insert_header(
        &mut headers,
        ACCEPT,
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    )?;
    Ok(headers)
}

/// 构造表单和上传请求头。
fn form_headers(
    session: &IbbLoginSession,
    referer: &str,
    accept: &str,
    urlencoded: bool,
) -> Result<HeaderMap> {
    let mut headers = base_headers(session)?;
    insert_header(&mut headers, REFERER, referer)?;
    insert_header(&mut headers, ORIGIN, &origin_from_url(referer)?)?;
    insert_header(&mut headers, ACCEPT, accept)?;
    if urlencoded {
        insert_header(
            &mut headers,
            CONTENT_TYPE,
            "application/x-www-form-urlencoded; charset=UTF-8",
        )?;
    }
    insert_header(
        &mut headers,
        reqwest::header::HeaderName::from_static("x-requested-with"),
        "XMLHttpRequest",
    )?;
    Ok(headers)
}

/// 构造 application/x-www-form-urlencoded 表单正文。
fn build_form_body(fields: Vec<(String, String)>) -> Result<String> {
    let url = Url::parse_with_params("https://imgbb.local/", fields)?;

    url.query()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("构造 ImgBB 表单失败"))
}

/// 从请求来源 URL 中推断 Origin。
fn origin_from_url(input: &str) -> Result<String> {
    let url = Url::parse(input).with_context(|| format!("解析请求来源失败: {input}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("请求来源缺少 host: {input}"))?;
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();

    Ok(format!("{}://{}{}", url.scheme(), host, port))
}

/// 构造登录态请求基础头。
fn base_headers(session: &IbbLoginSession) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, USER_AGENT, DEFAULT_BROWSER_USER_AGENT)?;
    insert_header(&mut headers, ACCEPT_LANGUAGE, DEFAULT_ACCEPT_LANGUAGE)?;
    insert_header(&mut headers, COOKIE, &session.cookie_header)?;
    Ok(headers)
}

/// 插入字符串请求头。
fn insert_header(
    headers: &mut HeaderMap,
    name: reqwest::header::HeaderName,
    value: &str,
) -> Result<()> {
    headers.insert(
        name,
        HeaderValue::from_str(value).with_context(|| format!("解析请求头失败: {value}"))?,
    );
    Ok(())
}

/// 创建管理接口 HTTP 客户端。
fn http_client() -> Client {
    Client::new()
}

/// 返回当前毫秒时间戳。
fn current_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证相册隐私值可以转换为接口字段。
    #[test]
    fn album_privacy_converts_to_form_value() {
        assert_eq!(IbbAlbumPrivacy::Password.as_form_value(), "password");
        assert_eq!(IbbAlbumPrivacy::Private.as_form_value(), "private");
        assert_eq!(IbbAlbumPrivacy::Public.as_form_value(), "public");
    }

    /// 验证图片 MIME 可以按扩展名推断。
    #[test]
    fn mime_can_be_detected_from_path() {
        assert_eq!(mime_from_path(Path::new("demo.png")), "image/png");
        assert_eq!(mime_from_path(Path::new("demo.jpg")), "image/jpeg");
        assert_eq!(
            mime_from_path(Path::new("demo.bin")),
            "application/octet-stream"
        );
    }

    /// 验证上传来源可以按相册地址推断 Origin。
    #[test]
    fn origin_can_be_detected_from_referer() {
        assert_eq!(
            origin_from_url("https://ibb.co/album/ABC123").unwrap(),
            "https://ibb.co"
        );
        assert_eq!(
            origin_from_url("https://demo.imgbb.com/albums?list=albums").unwrap(),
            "https://demo.imgbb.com"
        );
    }

    /// 验证图片管理 ID 可以兼容旧版图片直链。
    #[test]
    fn image_management_id_accepts_direct_image_url() {
        assert_eq!(
            normalize_image_management_id("https://i.ibb.co/ABC123/demo.jpg"),
            "ABC123"
        );
        assert_eq!(
            normalize_image_management_id("https://ibb.co/IMG456"),
            "IMG456"
        );
        assert_eq!(normalize_image_management_id("IMG789"), "IMG789");
    }
}
