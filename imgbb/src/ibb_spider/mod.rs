mod album;
mod name_generator;
mod profile;
mod utils;

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, ensure};
use llpha::*;
use name_generator::AlbumFileNameMode;
use profile::{
    IbbProfileUrl, extract_next_seek, normalize_profile_url as normalize_profile_url_input,
    parse_profile_albums,
};
use serde_json::Value;
use tracing::info;
use utils::extract_auth_token;

pub use album::{
    IbbAlbumDetail, IbbAlbumImage, IbbDownloadProgress, IbbDownloadProgressCallback,
    IbbDownloadProgressEvent, IbbDownloadProgressFuture, IbbSpiderReport,
};
pub use profile::{IbbProfileAlbum, IbbProfileBatch, IbbProfileReport};

/// IbbSpiderManager 统一管理 ImgBB 相册和用户主页任务。
pub struct IbbSpiderManager {
    client: Arc<LlphaClient>,
    options: IbbSpiderOptions,
}

impl IbbSpiderManager {
    /// 使用全局客户端创建 ImgBB 任务管理器。
    pub fn new() -> Self {
        Self {
            client: LlphaClient::global(),
            options: IbbSpiderOptions::default(),
        }
    }

    /// 设置下载基础目录。
    pub fn with_base_path(mut self, base_path: impl Into<PathBuf>) -> Self {
        self.options.base_path = base_path.into();
        self
    }

    /// 设置相册文件计数命名模板。
    pub fn with_file_name_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.options.file_name_mode = AlbumFileNameMode::CountPattern(pattern.into());
        self
    }

    /// 执行相册 JSON 抓取和内容下载。
    pub async fn download_album(&self, input_url: impl AsRef<str>) -> Result<IbbSpiderReport> {
        album::download_album(
            self.client.clone(),
            self.options.base_path.clone(),
            self.options.file_name_mode.clone(),
            input_url.as_ref(),
        )
        .await
    }

    /// 执行相册内容下载，并在下载过程中回调进度。
    pub async fn download_album_with_progress<F, Fut>(
        &self,
        input_url: impl AsRef<str>,
        progress: F,
    ) -> Result<IbbSpiderReport>
    where
        F: Fn(IbbDownloadProgress) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        album::download_album_with_progress(
            self.client.clone(),
            self.options.base_path.clone(),
            self.options.file_name_mode.clone(),
            input_url.as_ref(),
            Some(Arc::new(move |event| Box::pin(progress(event)))),
        )
        .await
    }

    /// 下载已解析相册中的指定图片。
    pub async fn download_album_images(
        &self,
        detail: &IbbAlbumDetail,
        image_ids: &[String],
    ) -> Result<IbbSpiderReport> {
        album::download_album_images(
            self.client.clone(),
            self.options.base_path.clone(),
            self.options.file_name_mode.clone(),
            detail,
            image_ids,
        )
        .await
    }

    /// 下载已解析相册中的指定图片，并在下载过程中回调进度。
    pub async fn download_album_images_with_progress<F, Fut>(
        &self,
        detail: &IbbAlbumDetail,
        image_ids: &[String],
        progress: F,
    ) -> Result<IbbSpiderReport>
    where
        F: Fn(IbbDownloadProgress) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        album::download_album_images_with_progress(
            self.client.clone(),
            self.options.base_path.clone(),
            self.options.file_name_mode.clone(),
            detail,
            image_ids,
            Some(Arc::new(move |event| Box::pin(progress(event)))),
        )
        .await
    }

    /// 解析相册信息但不执行文件下载。
    pub async fn parse_album(&self, input_url: impl AsRef<str>) -> Result<IbbAlbumDetail> {
        album::parse_album(self.client.clone(), input_url.as_ref()).await
    }

    /// 规整 ImgBB 相册 URL。
    pub fn normalize_album_url(input_url: impl AsRef<str>) -> Result<String> {
        album::normalize_album_url(input_url.as_ref())
    }

    /// 规整 ImgBB 个人空间 URL。
    pub fn normalize_profile_url(input_url: impl AsRef<str>) -> Result<String> {
        normalize_profile_url_input(input_url.as_ref())
    }

    /// 遍历 ImgBB 用户主页并返回全部子专辑。
    pub async fn list_profile_albums(
        &self,
        input_url: impl AsRef<str>,
    ) -> Result<IbbProfileReport> {
        self.stream_profile_albums(input_url, |_| async { Ok(()) })
            .await
    }

    /// 流式遍历 ImgBB 用户主页，每解析一批专辑就调用回调。
    pub async fn stream_profile_albums<F, Fut>(
        &self,
        input_url: impl AsRef<str>,
        mut on_batch: F,
    ) -> Result<IbbProfileReport>
    where
        F: FnMut(IbbProfileBatch) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        let profile = IbbProfileUrl::parse(input_url.as_ref())?;
        info!(url = %profile.normalized_url, "开始遍历 ImgBB 用户主页专辑");

        let initial_html = self.fetch_profile_html(&profile).await?;
        let auth_token = extract_auth_token(&initial_html)?;
        let mut albums = parse_profile_albums(&initial_html)?;
        if !albums.is_empty() {
            on_batch(IbbProfileBatch {
                page: 1,
                albums: albums.clone(),
                finished: false,
            })
            .await?;
        }

        let mut seen_urls = albums
            .iter()
            .map(|album| album.url.clone())
            .collect::<std::collections::HashSet<_>>();
        let mut page = 2usize;
        let mut seek = extract_next_seek(&initial_html);

        loop {
            let Some(current_seek) = seek.take().filter(|value| !value.is_empty()) else {
                break;
            };
            let response_json = self
                .fetch_profile_albums_json(&profile, &auth_token, page, &current_seek)
                .await?;
            let page_html = response_json
                .get("html")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let page_albums = parse_profile_albums(page_html)?;

            if page_albums.is_empty() {
                break;
            }

            let mut new_album_count = 0usize;
            let mut new_albums = Vec::new();
            for album in page_albums {
                if seen_urls.insert(album.url.clone()) {
                    new_albums.push(album.clone());
                    albums.push(album);
                    new_album_count = new_album_count.saturating_add(1);
                }
            }

            if new_album_count == 0 {
                break;
            }

            on_batch(IbbProfileBatch {
                page,
                albums: new_albums,
                finished: false,
            })
            .await?;

            page = page.saturating_add(1);
            seek = response_json
                .get("seekEnd")
                .and_then(Value::as_str)
                .map(str::to_string);
        }

        on_batch(IbbProfileBatch {
            page: page.saturating_sub(1).max(1),
            albums: Vec::new(),
            finished: true,
        })
        .await?;

        info!(album_count = albums.len(), "ImgBB 用户主页专辑遍历完成");

        Ok(IbbProfileReport { albums })
    }

    /// 拉取 ImgBB 用户主页相册列表首屏。
    async fn fetch_profile_html(&self, profile: &IbbProfileUrl) -> Result<String> {
        let request = FetchRequest::get(profile.normalized_url.clone())
            .with_headers(browser_page_headers(&profile.origin)?);
        let response = self.client.fetch(request).await?;
        ensure!(
            response.is_success(),
            "ImgBB 用户主页请求失败: {} {}",
            response.status,
            response.url
        );

        Ok(response.body)
    }

    /// 调用 ImgBB 用户主页增量加载接口。
    async fn fetch_profile_albums_json(
        &self,
        profile: &IbbProfileUrl,
        auth_token: &str,
        page: usize,
        seek: &str,
    ) -> Result<Value> {
        let body = profile.build_albums_json_body(auth_token, page, seek)?;
        let response = self
            .client
            .fetch(
                FetchRequest::post(profile.json_url.clone(), body).with_headers(
                    browser_form_headers(&profile.normalized_url, &profile.origin)?,
                ),
            )
            .await?;
        ensure!(
            response.is_success(),
            "ImgBB 用户主页 JSON 请求失败: {} {}",
            response.status,
            response.url
        );

        let response_json: Value =
            serde_json::from_str(&response.body).context("解析 ImgBB 用户主页 JSON 响应失败")?;
        ensure!(
            response_json
                .get("status_code")
                .and_then(Value::as_u64)
                .unwrap_or_default()
                == 200,
            "ImgBB 用户主页 JSON 接口返回异常: {}",
            response_json
        );

        Ok(response_json)
    }
}

/// IbbSpiderOptions 保存 ImgBB 相册任务配置。
struct IbbSpiderOptions {
    base_path: PathBuf,
    file_name_mode: AlbumFileNameMode,
}

impl Default for IbbSpiderOptions {
    /// 创建默认 ImgBB 相册任务配置。
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("."),
            file_name_mode: AlbumFileNameMode::default(),
        }
    }
}
