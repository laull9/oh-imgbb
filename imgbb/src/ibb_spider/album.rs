use std::collections::HashSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow, ensure};
use llpha::*;
use reqwest::{
    Url,
    header::{COOKIE, HeaderMap},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs;
use tracing::info;

use super::login::IbbLoginSession;
use super::manage_response::find_image_id;
use super::name_generator::{AlbumFileNameGenerator, AlbumFileNameMode};
use super::utils::{extract_auth_token, normalize_url_input, sanitize_path_segment};

const IBB_API_URL: &str = "https://ibb.co/json";
const IBB_ORIGIN: &str = "https://ibb.co";

/// IbbSpiderReport 保存 ImgBB 相册任务结果。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IbbSpiderReport {
    pub normalized_url: String,
    pub author_url: Option<String>,
    pub download_summary: AlbumDownloadSummary,
}

/// IbbAlbumDetail 保存 ImgBB 相册解析结果。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IbbAlbumDetail {
    pub url: String,
    pub title: String,
    pub author_url: Option<String>,
    pub images: Vec<IbbAlbumImage>,
}

/// IbbAlbumImage 保存 ImgBB 相册中的单张图片。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IbbAlbumImage {
    pub id: String,
    pub filename: String,
    pub image_url: String,
    pub thumbnail_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_thumbnail_path: Option<String>,
    pub sort_index: usize,
}

/// AlbumDownloadSummary 保存相册下载完成后的统计信息。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AlbumDownloadSummary {
    pub directory: PathBuf,
    pub downloaded_files: usize,
    pub bytes_written: usize,
}

/// AlbumDownloadFile 保存单个相册文件下载信息。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AlbumDownloadFile {
    pub(super) url: String,
    pub(super) path: PathBuf,
    pub(super) headers: HeaderMap,
}

/// AlbumDownloadPlan 保存相册下载目录和文件列表。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AlbumDownloadPlan {
    pub(super) directory: PathBuf,
    pub(super) files: Vec<AlbumDownloadFile>,
}

/// IbbAlbumUrl 保存 ImgBB 相册 URL 的规整结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct IbbAlbumUrl {
    pub(super) id: String,
    pub(super) normalized_url: String,
}

/// IbbDownloadProgressEvent 表示相册下载进度事件类型。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IbbDownloadProgressEvent {
    TotalKnown,
    FileFinished,
}

/// IbbDownloadProgress 保存相册下载进度。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IbbDownloadProgress {
    pub album_url: String,
    pub album_title: String,
    pub total_files: usize,
    pub finished_files: usize,
    pub downloaded_files: usize,
    pub bytes_written: usize,
    pub event: IbbDownloadProgressEvent,
}

/// IbbDownloadProgressFuture 表示相册下载进度回调 future。
pub type IbbDownloadProgressFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// IbbDownloadProgressCallback 表示相册下载进度回调。
pub type IbbDownloadProgressCallback =
    Arc<dyn Fn(IbbDownloadProgress) -> IbbDownloadProgressFuture + Send + Sync + 'static>;

impl IbbAlbumUrl {
    /// 解析并规整 ImgBB 相册 URL。
    pub(super) fn parse(input: &str) -> Result<Self> {
        let input = normalize_url_input(input);
        let url = Url::parse(&input).with_context(|| format!("解析 URL 失败: {input}"))?;
        let host = url.host_str().unwrap_or_default();
        ensure!(
            host == "ibb.co" || host == "www.ibb.co",
            "仅支持 ibb.co 相册 URL: {input}"
        );

        let segments = url
            .path_segments()
            .ok_or_else(|| anyhow!("URL 缺少路径: {input}"))?
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        ensure!(
            segments.len() >= 2 && segments[0] == "album",
            "URL 不是 ImgBB 相册地址: {input}"
        );

        let id = segments[1].to_string();
        ensure!(!id.is_empty(), "ImgBB 相册 ID 为空: {input}");

        Ok(Self {
            normalized_url: format!("https://ibb.co/album/{id}/"),
            id,
        })
    }

    /// 返回相册 embeds 页面地址。
    pub(super) fn embeds_url(&self) -> String {
        format!("{}embeds", self.normalized_url)
    }

    /// 返回相册 embeds 页面的路径。
    pub(super) fn embeds_path(&self) -> String {
        format!("/album/{}/embeds", self.id)
    }
}

/// 执行相册 JSON 抓取、信息解析和文件下载。
pub(super) async fn download_album(
    client: Arc<LlphaClient>,
    base_path: PathBuf,
    file_name_mode: AlbumFileNameMode,
    input_url: &str,
) -> Result<IbbSpiderReport> {
    download_album_with_progress(client, base_path, file_name_mode, input_url, None).await
}

/// 执行相册 JSON 抓取、信息解析和文件下载，并推送进度。
pub(super) async fn download_album_with_progress(
    client: Arc<LlphaClient>,
    base_path: PathBuf,
    file_name_mode: AlbumFileNameMode,
    input_url: &str,
    progress: Option<IbbDownloadProgressCallback>,
) -> Result<IbbSpiderReport> {
    download_album_with_session(client, base_path, file_name_mode, input_url, None, progress).await
}

/// 使用登录态执行相册 JSON 抓取、信息解析和文件下载。
pub(super) async fn download_authenticated_album_with_progress(
    client: Arc<LlphaClient>,
    base_path: PathBuf,
    file_name_mode: AlbumFileNameMode,
    session: &IbbLoginSession,
    input_url: &str,
    progress: Option<IbbDownloadProgressCallback>,
) -> Result<IbbSpiderReport> {
    download_album_with_session(
        client,
        base_path,
        file_name_mode,
        input_url,
        Some(session),
        progress,
    )
    .await
}

/// 执行可选认证的相册下载流程。
async fn download_album_with_session(
    client: Arc<LlphaClient>,
    base_path: PathBuf,
    file_name_mode: AlbumFileNameMode,
    input_url: &str,
    session: Option<&IbbLoginSession>,
    progress: Option<IbbDownloadProgressCallback>,
) -> Result<IbbSpiderReport> {
    let album = IbbAlbumUrl::parse(input_url)?;
    info!(url = %album.normalized_url, "开始执行 ImgBB 相册任务");

    let (parsed_album, album_json) = fetch_album_detail_and_json(&client, &album, session).await?;
    let download_summary = download_album_contents(
        client,
        &base_path,
        &album_json,
        &file_name_mode,
        &album.normalized_url,
        session,
        progress,
    )
    .await?;

    info!(
        downloaded_files = download_summary.downloaded_files,
        bytes_written = download_summary.bytes_written,
        "ImgBB 相册任务完成"
    );

    Ok(IbbSpiderReport {
        normalized_url: album.normalized_url,
        author_url: parsed_album.author_url,
        download_summary,
    })
}

/// 下载相册中指定图片。
pub(super) async fn download_album_images(
    client: Arc<LlphaClient>,
    base_path: PathBuf,
    file_name_mode: AlbumFileNameMode,
    detail: &IbbAlbumDetail,
    image_ids: &[String],
) -> Result<IbbSpiderReport> {
    download_album_images_with_progress(client, base_path, file_name_mode, detail, image_ids, None)
        .await
}

/// 下载已解析相册中的指定图片，并推送进度。
pub(super) async fn download_album_images_with_progress(
    client: Arc<LlphaClient>,
    base_path: PathBuf,
    file_name_mode: AlbumFileNameMode,
    detail: &IbbAlbumDetail,
    image_ids: &[String],
    progress: Option<IbbDownloadProgressCallback>,
) -> Result<IbbSpiderReport> {
    download_album_images_with_session(
        client,
        base_path,
        file_name_mode,
        detail,
        image_ids,
        None,
        progress,
    )
    .await
}

/// 使用登录态下载已解析相册中的指定图片。
pub(super) async fn download_authenticated_album_images_with_progress(
    client: Arc<LlphaClient>,
    base_path: PathBuf,
    file_name_mode: AlbumFileNameMode,
    session: &IbbLoginSession,
    detail: &IbbAlbumDetail,
    image_ids: &[String],
    progress: Option<IbbDownloadProgressCallback>,
) -> Result<IbbSpiderReport> {
    download_album_images_with_session(
        client,
        base_path,
        file_name_mode,
        detail,
        image_ids,
        Some(session),
        progress,
    )
    .await
}

/// 执行可选认证的选中图片下载流程。
async fn download_album_images_with_session(
    client: Arc<LlphaClient>,
    base_path: PathBuf,
    file_name_mode: AlbumFileNameMode,
    detail: &IbbAlbumDetail,
    image_ids: &[String],
    session: Option<&IbbLoginSession>,
    progress: Option<IbbDownloadProgressCallback>,
) -> Result<IbbSpiderReport> {
    let album = IbbAlbumUrl::parse(&detail.url)?;
    let plan =
        build_selected_download_plan(&base_path, detail, image_ids, &file_name_mode, session)?;
    let download_summary = download_album_plan(
        client,
        plan,
        album.normalized_url.clone(),
        detail.title.clone(),
        progress,
    )
    .await?;

    Ok(IbbSpiderReport {
        normalized_url: album.normalized_url,
        author_url: detail.author_url.clone(),
        download_summary,
    })
}

/// 解析相册信息但不执行文件下载。
pub(super) async fn parse_album(
    client: Arc<LlphaClient>,
    input_url: &str,
) -> Result<IbbAlbumDetail> {
    let album = IbbAlbumUrl::parse(input_url)?;
    fetch_album_detail(&client, &album, None).await
}

/// 以登录态解析相册信息但不执行文件下载。
pub(super) async fn parse_authenticated_album(
    client: Arc<LlphaClient>,
    session: &IbbLoginSession,
    input_url: &str,
) -> Result<IbbAlbumDetail> {
    let album = IbbAlbumUrl::parse(input_url)?;
    fetch_album_detail(&client, &album, Some(session)).await
}

/// 规整相册 URL。
pub(super) fn normalize_album_url(input_url: &str) -> Result<String> {
    Ok(IbbAlbumUrl::parse(input_url)?.normalized_url)
}

/// 抓取相册页面和 JSON 并组装预览详情。
async fn fetch_album_detail(
    client: &LlphaClient,
    album: &IbbAlbumUrl,
    session: Option<&IbbLoginSession>,
) -> Result<IbbAlbumDetail> {
    Ok(fetch_album_detail_and_json(client, album, session).await?.0)
}

/// 抓取相册页面和 JSON 并返回详情与原始 JSON。
async fn fetch_album_detail_and_json(
    client: &LlphaClient,
    album: &IbbAlbumUrl,
    session: Option<&IbbLoginSession>,
) -> Result<(IbbAlbumDetail, Value)> {
    let album_html = fetch_album_html(client, album, session).await?;
    let author_url = extract_album_author_url(&album_html, &album.normalized_url)?;
    let album_json = fetch_album_json(client, album, session).await?;
    let title = required_json_string(&album_json, "/album/name")?.to_string();
    let images = parse_album_images(&album_json)?;

    Ok((
        IbbAlbumDetail {
            url: album.normalized_url.clone(),
            title,
            author_url,
            images,
        },
        album_json,
    ))
}

/// 抓取相册主页面用于读取相册附加信息。
async fn fetch_album_html(
    client: &LlphaClient,
    album: &IbbAlbumUrl,
    session: Option<&IbbLoginSession>,
) -> Result<String> {
    let mut headers = browser_page_headers(IBB_ORIGIN)?;
    if let Some(session) = session {
        insert_header(&mut headers, COOKIE, &session.cookie_header)?;
    }
    let request = FetchRequest::get(album.normalized_url.clone()).with_headers(headers);
    let response = client.fetch(request).await?;
    ensure!(
        response.is_success(),
        "ImgBB 相册页面请求失败: {} {}",
        response.status,
        response.url
    );

    Ok(response.body)
}

/// 抓取相册 embeds 页面并读取内容 JSON。
async fn fetch_album_json(
    client: &LlphaClient,
    album: &IbbAlbumUrl,
    session: Option<&IbbLoginSession>,
) -> Result<Value> {
    let embeds_html = fetch_embeds_html(client, album, session).await?;
    let auth_token = extract_auth_token(&embeds_html)?;
    let response_json = fetch_album_contents_json(client, album, session, &auth_token).await?;
    ensure!(
        response_json
            .get("status_code")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            == 200,
        "ImgBB JSON 接口返回异常: {}",
        response_json
    );

    let image_count = response_json
        .get("contents")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    info!(image_count, "ImgBB 相册 JSON 抓取完成");

    Ok(response_json)
}

/// 拉取相册 embeds 页面以提取动态 auth_token。
async fn fetch_embeds_html(
    client: &LlphaClient,
    album: &IbbAlbumUrl,
    session: Option<&IbbLoginSession>,
) -> Result<String> {
    let mut headers = browser_page_headers(&album.normalized_url)?;
    if let Some(session) = session {
        insert_header(&mut headers, COOKIE, &session.cookie_header)?;
    }
    let response = client
        .fetch(FetchRequest::get(album.embeds_url()).with_headers(headers))
        .await?;
    ensure!(
        response.is_success(),
        "ImgBB embeds 页面请求失败: {} {}",
        response.status,
        response.url
    );

    Ok(response.body)
}

/// 调用 ImgBB JSON 接口读取相册图片数据。
async fn fetch_album_contents_json(
    client: &LlphaClient,
    album: &IbbAlbumUrl,
    session: Option<&IbbLoginSession>,
    auth_token: &str,
) -> Result<Value> {
    let body = build_album_json_body(album, auth_token)?;
    let mut headers = browser_form_headers(&album.embeds_url(), IBB_ORIGIN)?;
    if let Some(session) = session {
        insert_header(&mut headers, COOKIE, &session.cookie_header)?;
    }
    let request = FetchRequest::post(IBB_API_URL, body).with_headers(headers);
    let response = client.fetch(request).await?;
    ensure!(
        response.is_success(),
        "ImgBB JSON 请求失败: {} {}",
        response.status,
        response.url
    );

    serde_json::from_str(&response.body).context("解析 ImgBB JSON 响应失败")
}

/// 从相册 JSON 中解析图片预览列表。
fn parse_album_images(album_json: &Value) -> Result<Vec<IbbAlbumImage>> {
    let contents = required_json_array(album_json, "/contents")?;
    let mut images = Vec::with_capacity(contents.len());

    for (index, item) in contents.iter().enumerate() {
        let filename = required_json_item_string(item, "contents", index, "filename")?;
        let image_url = required_json_item_string(item, "contents", index, "url")?;
        let image_id = find_image_id(item)
            .or_else(|| image_id_from_url(image_url))
            .unwrap_or_else(|| image_url.to_string());
        let thumbnail_url = optional_image_variant_url(item, "thumb")
            .or_else(|| optional_image_variant_url(item, "display_url"))
            .or_else(|| optional_image_variant_url(item, "medium"));

        images.push(IbbAlbumImage {
            id: image_id,
            filename: filename.to_string(),
            image_url: image_url.to_string(),
            thumbnail_url,
            local_thumbnail_path: None,
            sort_index: index,
        });
    }

    Ok(images)
}

/// 从图片直链中提取 ImgBB encoded ID。
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

/// 从图片变体字段中读取 URL。
fn optional_image_variant_url(item: &Value, key: &str) -> Option<String> {
    let value = item.get(key)?;
    let url = match value {
        Value::String(url) => Some(url.as_str()),
        Value::Object(object) => object.get("url").and_then(Value::as_str),
        _ => None,
    }?;

    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    Some(url.to_string())
}

/// 构造 ImgBB 相册内容接口的表单正文。
fn build_album_json_body(album: &IbbAlbumUrl, auth_token: &str) -> Result<String> {
    let url = Url::parse_with_params(
        IBB_API_URL,
        [
            ("action", "get-album-contents"),
            ("albumid", album.id.as_str()),
            ("auth_token", auth_token),
            ("pathname", album.embeds_path().as_str()),
        ],
    )?;

    url.query()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("构造 ImgBB JSON 表单失败"))
}

/// 下载相册 JSON 中声明的所有文件。
async fn download_album_contents(
    client: Arc<LlphaClient>,
    base_path: &Path,
    album_json: &Value,
    file_name_mode: &AlbumFileNameMode,
    album_url: &str,
    session: Option<&IbbLoginSession>,
    progress: Option<IbbDownloadProgressCallback>,
) -> Result<AlbumDownloadSummary> {
    let plan = build_download_plan(base_path, album_json, file_name_mode, album_url, session)?;
    let album_title = required_json_string(album_json, "/album/name")?.to_string();

    download_album_plan(client, plan, album_url.to_string(), album_title, progress).await
}

/// 按下载计划执行所有文件下载。
async fn download_album_plan(
    client: Arc<LlphaClient>,
    plan: AlbumDownloadPlan,
    album_url: String,
    album_title: String,
    progress: Option<IbbDownloadProgressCallback>,
) -> Result<AlbumDownloadSummary> {
    fs::create_dir_all(&plan.directory)
        .await
        .with_context(|| format!("创建相册目录失败: {}", plan.directory.display()))?;

    let pool = TaskPool::new(client.max_concurrent_requests());
    let directory = plan.directory.clone();
    let total_files = plan.files.len();
    let finished_files = Arc::new(AtomicUsize::new(0));

    emit_album_download_progress(
        &progress,
        IbbDownloadProgress {
            album_url: album_url.clone(),
            album_title: album_title.clone(),
            total_files,
            finished_files: 0,
            downloaded_files: 0,
            bytes_written: 0,
            event: IbbDownloadProgressEvent::TotalKnown,
        },
    )
    .await;

    let report = pool
        .run_all(plan.files, move |file| {
            let client = client.clone();
            let progress = progress.clone();
            let album_url = album_url.clone();
            let album_title = album_title.clone();
            let finished_files = finished_files.clone();
            async move {
                let url = file.url;
                let path = file.path;
                let request = FetchRequest::get(url.clone()).with_headers(file.headers);
                info!(url = %url, path = %path.display(), "开始下载相册文件");
                let saved = client.download_request_to_file(request, &path).await?;
                let finished_files = finished_files.fetch_add(1, Ordering::SeqCst) + 1;
                emit_album_download_progress(
                    &progress,
                    IbbDownloadProgress {
                        album_url,
                        album_title,
                        total_files,
                        finished_files,
                        downloaded_files: finished_files,
                        bytes_written: saved.bytes_written,
                        event: IbbDownloadProgressEvent::FileFinished,
                    },
                )
                .await;
                info!(path = %saved.path.display(), bytes = saved.bytes_written, "相册文件下载完成");

                Ok(saved)
            }
        })
        .await;

    collect_download_results(report, directory)
}

/// 执行可选的相册下载进度回调。
async fn emit_album_download_progress(
    callback: &Option<IbbDownloadProgressCallback>,
    progress: IbbDownloadProgress,
) {
    if let Some(callback) = callback {
        callback(progress).await;
    }
}

/// 从已解析相册构造选中图片下载计划。
fn build_selected_download_plan(
    base_path: &Path,
    detail: &IbbAlbumDetail,
    image_ids: &[String],
    file_name_mode: &AlbumFileNameMode,
    session: Option<&IbbLoginSession>,
) -> Result<AlbumDownloadPlan> {
    ensure!(!image_ids.is_empty(), "请选择要下载的图片");

    let directory_name = sanitize_path_segment(&detail.title);
    ensure!(!directory_name.is_empty(), "相册名称为空");

    let selected_ids = image_ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let directory = base_path.join(directory_name);
    let mut generator =
        AlbumFileNameGenerator::new(directory.clone(), &detail.title, file_name_mode.clone())?;
    let mut files = Vec::new();
    let headers = image_headers_for_album(&detail.url, session)?;

    for image in detail
        .images
        .iter()
        .filter(|image| selected_ids.contains(image.id.as_str()))
    {
        let path = generator
            .next_path(&image.filename)
            .with_context(|| format!("生成 {} 目标路径失败", image.filename))?;
        files.push(AlbumDownloadFile {
            url: image.image_url.clone(),
            path,
            headers: headers.clone(),
        });
    }

    ensure!(!files.is_empty(), "选中的图片不在相册中");

    Ok(AlbumDownloadPlan { directory, files })
}

/// 从相册 JSON 构造下载计划。
fn build_download_plan(
    base_path: &Path,
    album_json: &Value,
    file_name_mode: &AlbumFileNameMode,
    album_url: &str,
    session: Option<&IbbLoginSession>,
) -> Result<AlbumDownloadPlan> {
    let album_name = required_json_string(album_json, "/album/name")?;
    let directory_name = sanitize_path_segment(album_name);
    ensure!(!directory_name.is_empty(), "相册名称为空");

    let contents = required_json_array(album_json, "/contents")?;
    let directory = base_path.join(directory_name);
    let mut generator =
        AlbumFileNameGenerator::new(directory.clone(), album_name, file_name_mode.clone())?;
    let mut files = Vec::with_capacity(contents.len());
    let headers = image_headers_for_album(album_url, session)?;

    for (index, item) in contents.iter().enumerate() {
        let filename = required_json_item_string(item, "contents", index, "filename")?;
        let url = required_json_item_string(item, "contents", index, "url")?;
        let path = generator
            .next_path(filename)
            .with_context(|| format!("生成 contents[{index}].filename 目标路径失败"))?;

        files.push(AlbumDownloadFile {
            url: url.to_string(),
            path,
            headers: headers.clone(),
        });
    }

    Ok(AlbumDownloadPlan { directory, files })
}

/// 构造相册图片下载请求头，登录态存在时携带 Cookie。
fn image_headers_for_album(
    album_url: &str,
    session: Option<&IbbLoginSession>,
) -> Result<HeaderMap> {
    let mut headers = image_download_headers(album_url)?;
    if let Some(session) = session {
        insert_header(&mut headers, COOKIE, &session.cookie_header)?;
    }

    Ok(headers)
}

/// 汇总并检查所有下载任务结果。
fn collect_download_results(
    report: TaskPoolReport<SavedDownload>,
    directory: PathBuf,
) -> Result<AlbumDownloadSummary> {
    let mut downloaded_files = 0usize;
    let mut bytes_written = 0usize;

    ensure!(
        report.failures.is_empty(),
        "部分文件下载失败:\n{}",
        report.failures.join("\n")
    );

    for saved in report.successes {
        downloaded_files = downloaded_files.saturating_add(1);
        bytes_written = bytes_written.saturating_add(saved.bytes_written);
    }

    Ok(AlbumDownloadSummary {
        directory,
        downloaded_files,
        bytes_written,
    })
}

/// 从相册主页面提取作者 albums 地址。
fn extract_album_author_url(html: &str, base_url: &str) -> Result<Option<String>> {
    let Some(href) = html_attr(
        html,
        "#album > div.content-width > div:nth-child(2) > div.header-content-left > div > div > a",
        "href",
    )?
    else {
        return Ok(None);
    };

    normalize_author_albums_url(base_url, &href).map(Some)
}

/// 规整作者地址并补齐 albums 后缀。
fn normalize_author_albums_url(base_url: &str, href: &str) -> Result<String> {
    let base = Url::parse(base_url).with_context(|| format!("解析相册 URL 失败: {base_url}"))?;
    let mut url = base
        .join(href)
        .with_context(|| format!("解析作者 URL 失败: {href}"))?;
    let path = url.path().trim_end_matches('/');

    if !path.ends_with("/albums") {
        let mut new_path = path.to_string();
        if !new_path.ends_with('/') {
            new_path.push('/');
        }
        new_path.push_str("albums");
        url.set_path(&new_path);
    }
    url.set_fragment(None);

    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证相册 URL 可以补全协议并规整。
    #[test]
    fn album_url_normalizes_input() {
        let album = IbbAlbumUrl::parse("ibb.co/album/ABC123").unwrap();

        assert_eq!(album.id, "ABC123");
        assert_eq!(album.normalized_url, "https://ibb.co/album/ABC123/");
        assert_eq!(album.embeds_path(), "/album/ABC123/embeds");
    }

    /// 验证下载计划默认使用当前目录作为基础目录。
    #[test]
    fn download_plan_uses_current_dir_by_default() {
        let plan = build_download_plan(
            &PathBuf::from("."),
            &sample_album_json(),
            &AlbumFileNameMode::default(),
            "https://ibb.co/album/ABC123/",
            None,
        )
        .unwrap();

        assert_eq!(plan.directory, PathBuf::from(".").join("demo_album"));
        assert_eq!(
            plan.files[0].path,
            PathBuf::from(".").join("demo_album").join("a_b.jpg")
        );
        assert_eq!(
            plan.files[1].path,
            PathBuf::from(".").join("demo_album").join("a_b_1.jpg")
        );
    }

    /// 验证下载计划支持自定义基础目录。
    #[test]
    fn download_plan_accepts_base_path() {
        let plan = build_download_plan(
            &PathBuf::from("downloads"),
            &sample_album_json(),
            &AlbumFileNameMode::default(),
            "https://ibb.co/album/ABC123/",
            None,
        )
        .unwrap();

        assert_eq!(
            plan.directory,
            PathBuf::from("downloads").join("demo_album")
        );
        assert_eq!(
            plan.files[0].path,
            PathBuf::from("downloads")
                .join("demo_album")
                .join("a_b.jpg")
        );
    }

    /// 验证下载计划支持计数命名模板。
    #[test]
    fn download_plan_accepts_count_pattern() {
        let plan = build_download_plan(
            &PathBuf::from("downloads"),
            &sample_album_json(),
            &AlbumFileNameMode::CountPattern("{album}_{count}_{name}".to_string()),
            "https://ibb.co/album/ABC123/",
            None,
        )
        .unwrap();

        assert_eq!(
            plan.files[0].path,
            PathBuf::from("downloads")
                .join("demo_album")
                .join("demo_album_1_a_b.jpg")
        );
        assert_eq!(
            plan.files[1].path,
            PathBuf::from("downloads")
                .join("demo_album")
                .join("demo_album_2_a_b.jpg")
        );
    }

    /// 验证相册 JSON 可以解析为预览图片列表。
    #[test]
    fn album_images_parse_from_json() {
        let images = parse_album_images(&sample_album_json()).unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].id, "IMG123");
        assert_eq!(images[1].id, "IMG456");
        assert_eq!(images[0].filename, "a:b.jpg");
        assert_eq!(images[0].image_url, "https://i.ibb.co/a.jpg");
        assert_eq!(
            images[0].thumbnail_url,
            Some("https://i.ibb.co/thumb/a.jpg".to_string())
        );
        assert_eq!(
            images[1].thumbnail_url,
            Some("https://i.ibb.co/medium/a-copy.jpg".to_string())
        );
        assert_eq!(images[0].sort_index, 0);
    }

    /// 验证选中图片下载计划只包含被选中的图片。
    #[test]
    fn selected_download_plan_keeps_selected_images() {
        let detail = sample_album_detail();
        let plan = build_selected_download_plan(
            &PathBuf::from("downloads"),
            &detail,
            &[detail.images[1].id.clone()],
            &AlbumFileNameMode::CountPattern("{count}_{name}".to_string()),
            None,
        )
        .unwrap();

        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].url, "https://i.ibb.co/a-copy.jpg");
        assert_eq!(
            plan.files[0].path,
            PathBuf::from("downloads")
                .join("demo_album")
                .join("1_a_b.jpg")
        );
    }

    /// 验证相册主页面可以提取并补齐作者 albums 地址。
    #[test]
    fn album_author_url_appends_albums_suffix() {
        let author_url = extract_album_author_url(
            &sample_album_html("https://beautif11.imgbb.com/"),
            "https://ibb.co/album/ABC123/",
        )
        .unwrap();

        assert_eq!(
            author_url,
            Some("https://beautif11.imgbb.com/albums".to_string())
        );
    }

    /// 验证已有 albums 后缀的作者地址不会重复追加。
    #[test]
    fn album_author_url_keeps_existing_albums_suffix() {
        let author_url = extract_album_author_url(
            &sample_album_html("https://beautif11.imgbb.com/albums"),
            "https://ibb.co/album/ABC123/",
        )
        .unwrap();

        assert_eq!(
            author_url,
            Some("https://beautif11.imgbb.com/albums".to_string())
        );
    }

    /// 构造测试用相册 JSON。
    fn sample_album_json() -> Value {
        json!({
            "album": {
                "name": "demo/album"
            },
            "contents": [
                {
                    "id_encoded": "IMG123",
                    "filename": "a:b.jpg",
                    "url": "https://i.ibb.co/a.jpg",
                    "thumb": {
                        "url": "https://i.ibb.co/thumb/a.jpg"
                    }
                },
                {
                    "image": {
                        "id_encoded": "IMG456"
                    },
                    "filename": "a:b.jpg",
                    "url": "https://i.ibb.co/a-copy.jpg",
                    "medium": {
                        "url": "https://i.ibb.co/medium/a-copy.jpg"
                    }
                }
            ]
        })
    }

    /// 构造测试用相册详情。
    fn sample_album_detail() -> IbbAlbumDetail {
        IbbAlbumDetail {
            url: "https://ibb.co/album/ABC123/".to_string(),
            title: "demo/album".to_string(),
            author_url: Some("https://beautif11.imgbb.com/albums".to_string()),
            images: parse_album_images(&sample_album_json()).unwrap(),
        }
    }

    /// 构造测试用相册页面 HTML。
    fn sample_album_html(author_url: &str) -> String {
        format!(
            r#"
            <div id="album">
                <div class="content-width">
                    <div></div>
                    <div>
                        <div class="header-content-left">
                            <div>
                                <div><a href="{author_url}">author</a></div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
            "#
        )
    }
}
