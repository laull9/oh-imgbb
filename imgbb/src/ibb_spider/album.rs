use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, ensure};
use llpha::*;
use reqwest::Url;
use scraper::{Html, Selector};
use serde_json::Value;
use tokio::fs;
use tracing::info;

use super::utils::{extract_auth_token, normalize_url_input, sanitize_path_segment};

const IBB_API_URL: &str = "https://ibb.co/json";
const IBB_ORIGIN: &str = "https://ibb.co";

/// IbbSpiderReport 保存 ImgBB 相册任务结果。
pub struct IbbSpiderReport {
    pub normalized_url: String,
    pub author_url: Option<String>,
    pub download_summary: AlbumDownloadSummary,
}

/// AlbumDownloadSummary 保存相册下载完成后的统计信息。
#[derive(Clone, Debug, Eq, PartialEq)]
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
    input_url: &str,
) -> Result<IbbSpiderReport> {
    let album = IbbAlbumUrl::parse(input_url)?;
    info!(url = %album.normalized_url, "开始执行 ImgBB 相册任务");

    let album_html = fetch_album_html(&client, &album).await?;
    let author_url = extract_album_author_url(&album_html, &album.normalized_url)?;
    let album_json = fetch_album_json(&client, &album).await?;
    let download_summary = download_album_contents(client, &base_path, &album_json).await?;

    info!(
        downloaded_files = download_summary.downloaded_files,
        bytes_written = download_summary.bytes_written,
        "ImgBB 相册任务完成"
    );

    Ok(IbbSpiderReport {
        normalized_url: album.normalized_url,
        author_url,
        download_summary,
    })
}

/// 抓取相册主页面用于读取相册附加信息。
async fn fetch_album_html(client: &LlphaClient, album: &IbbAlbumUrl) -> Result<String> {
    let request = FetchRequest::get(album.normalized_url.clone())
        .with_headers(browser_page_headers(IBB_ORIGIN)?);
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
async fn fetch_album_json(client: &LlphaClient, album: &IbbAlbumUrl) -> Result<Value> {
    let embeds_html = fetch_embeds_html(client, album).await?;
    let auth_token = extract_auth_token(&embeds_html)?;
    let response_json = fetch_album_contents_json(client, album, &auth_token).await?;
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
async fn fetch_embeds_html(client: &LlphaClient, album: &IbbAlbumUrl) -> Result<String> {
    let response = client.fetch(album.embeds_url()).await?;
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
    auth_token: &str,
) -> Result<Value> {
    let body = build_album_json_body(album, auth_token)?;
    let request = FetchRequest::post(IBB_API_URL, body)
        .with_headers(browser_form_headers(&album.embeds_url(), IBB_ORIGIN)?);
    let response = client.fetch(request).await?;
    ensure!(
        response.is_success(),
        "ImgBB JSON 请求失败: {} {}",
        response.status,
        response.url
    );

    serde_json::from_str(&response.body).context("解析 ImgBB JSON 响应失败")
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
    base_path: &PathBuf,
    album_json: &Value,
) -> Result<AlbumDownloadSummary> {
    let plan = build_download_plan(base_path, album_json)?;
    fs::create_dir_all(&plan.directory)
        .await
        .with_context(|| format!("创建相册目录失败: {}", plan.directory.display()))?;

    let pool = TaskPool::new(client.max_concurrent_requests());
    let directory = plan.directory.clone();

    let report = pool
        .run_all(plan.files, move |file| {
            let client = client.clone();
            async move {
                let url = file.url;
                let path = file.path;
                info!(url = %url, path = %path.display(), "开始下载相册文件");
                let saved = client.download_file(url, &path).await?;
                info!(path = %saved.path.display(), bytes = saved.bytes_written, "相册文件下载完成");

                Ok(saved)
            }
        })
        .await;

    collect_download_results(report, directory)
}

/// 从相册 JSON 构造下载计划。
fn build_download_plan(base_path: &PathBuf, album_json: &Value) -> Result<AlbumDownloadPlan> {
    let album_name = required_json_string(album_json, "/album/name")?;
    let directory_name = sanitize_path_segment(album_name);
    ensure!(!directory_name.is_empty(), "相册名称为空");

    let contents = required_json_array(album_json, "/contents")?;
    let directory = base_path.join(directory_name);
    let mut files = Vec::with_capacity(contents.len());

    for (index, item) in contents.iter().enumerate() {
        let filename = required_json_item_string(item, "contents", index, "filename")?;
        let url = required_json_item_string(item, "contents", index, "url")?;
        let safe_filename = sanitize_path_segment(filename);
        ensure!(!safe_filename.is_empty(), "contents[{index}].filename 为空");

        files.push(AlbumDownloadFile {
            url: url.to_string(),
            path: directory.join(safe_filename),
        });
    }

    Ok(AlbumDownloadPlan { directory, files })
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
    let document = Html::parse_document(html);
    let selector = parse_selector(
        "#album > div.content-width > div:nth-child(2) > div.header-content-left > div > div > a",
    )?;
    let Some(href) = document
        .select(&selector)
        .next()
        .and_then(|link| link.value().attr("href"))
        .map(str::trim)
        .filter(|href| !href.is_empty())
    else {
        return Ok(None);
    };

    normalize_author_albums_url(base_url, href).map(Some)
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

/// 解析 CSS 选择器并转换错误类型。
fn parse_selector(selector: &str) -> Result<Selector> {
    Selector::parse(selector).map_err(|err| anyhow!("CSS 选择器解析失败 {selector}: {err}"))
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
        let plan = build_download_plan(&PathBuf::from("."), &sample_album_json()).unwrap();

        assert_eq!(plan.directory, PathBuf::from(".").join("demo_album"));
        assert_eq!(
            plan.files[0].path,
            PathBuf::from(".").join("demo_album").join("a_b.jpg")
        );
    }

    /// 验证下载计划支持自定义基础目录。
    #[test]
    fn download_plan_accepts_base_path() {
        let plan = build_download_plan(&PathBuf::from("downloads"), &sample_album_json()).unwrap();

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
                    "filename": "a:b.jpg",
                    "url": "https://i.ibb.co/a.jpg"
                }
            ]
        })
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
