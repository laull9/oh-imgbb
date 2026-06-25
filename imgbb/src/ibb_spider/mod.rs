mod album;
mod login;
mod manage;
mod manage_response;
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
use reqwest::header::COOKIE;
use serde_json::Value;
use tracing::info;
use utils::extract_auth_token;

pub use album::{
    IbbAlbumDetail, IbbAlbumImage, IbbDownloadProgress, IbbDownloadProgressCallback,
    IbbDownloadProgressEvent, IbbDownloadProgressFuture, IbbSpiderReport,
};
pub use login::{IbbAuthenticatedProfile, IbbCookie, IbbLoginSession};
pub use manage::{IbbAlbumPrivacy, IbbApiReport, IbbCreateAlbumInput, IbbEditImageInput};
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

    /// 以登录态解析相册信息但不执行文件下载。
    pub async fn parse_authenticated_album(
        &self,
        session: &IbbLoginSession,
        input_url: impl AsRef<str>,
    ) -> Result<IbbAlbumDetail> {
        album::parse_authenticated_album(self.client.clone(), session, input_url.as_ref()).await
    }

    /// 登录 ImgBB 并返回内存会话信息。
    pub async fn login(
        &self,
        login_subject: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<IbbLoginSession> {
        login::login(
            self.client.clone(),
            login_subject.as_ref(),
            password.as_ref(),
        )
        .await
    }

    /// 创建已登录账号下的新相册。
    pub async fn create_album(
        &self,
        session: &IbbLoginSession,
        input: IbbCreateAlbumInput,
    ) -> Result<IbbApiReport> {
        manage::create_album(session, input).await
    }

    /// 上传文件到指定相册。
    pub async fn upload_album_image(
        &self,
        session: &IbbLoginSession,
        album_id: impl AsRef<str>,
        file_path: impl AsRef<std::path::Path>,
    ) -> Result<IbbApiReport> {
        manage::upload_album_image(session, album_id.as_ref(), file_path.as_ref()).await
    }

    /// 删除指定图片。
    pub async fn delete_image(
        &self,
        session: &IbbLoginSession,
        image_id: impl AsRef<str>,
    ) -> Result<IbbApiReport> {
        manage::delete_image(session, image_id.as_ref()).await
    }

    /// 删除指定相册。
    pub async fn delete_album(
        &self,
        session: &IbbLoginSession,
        album_id: impl AsRef<str>,
    ) -> Result<IbbApiReport> {
        manage::delete_album(session, album_id.as_ref()).await
    }

    /// 上传个人主页背景图。
    pub async fn upload_profile_background(
        &self,
        session: &IbbLoginSession,
        file_path: impl AsRef<std::path::Path>,
    ) -> Result<IbbApiReport> {
        manage::upload_profile_background(session, file_path.as_ref()).await
    }

    /// 删除个人主页背景图。
    pub async fn delete_profile_background(
        &self,
        session: &IbbLoginSession,
    ) -> Result<IbbApiReport> {
        manage::delete_profile_background(session).await
    }

    /// 编辑图片标题、描述或所属相册。
    pub async fn edit_image(
        &self,
        session: &IbbLoginSession,
        input: IbbEditImageInput,
    ) -> Result<IbbApiReport> {
        manage::edit_image(session, input).await
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

    /// 遍历已登录账号视角下的 ImgBB 用户主页专辑。
    pub async fn list_authenticated_profile_albums(
        &self,
        session: &IbbLoginSession,
        input_url: impl AsRef<str>,
    ) -> Result<IbbProfileReport> {
        self.stream_authenticated_profile_albums(session, input_url, |_| async { Ok(()) })
            .await
    }

    /// 流式遍历 ImgBB 用户主页，每解析一批专辑就调用回调。
    pub async fn stream_profile_albums<F, Fut>(
        &self,
        input_url: impl AsRef<str>,
        on_batch: F,
    ) -> Result<IbbProfileReport>
    where
        F: FnMut(IbbProfileBatch) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        self.stream_profile_albums_with_session(input_url, None, on_batch)
            .await
    }

    /// 流式遍历已登录账号视角下的 ImgBB 用户主页专辑。
    pub async fn stream_authenticated_profile_albums<F, Fut>(
        &self,
        session: &IbbLoginSession,
        input_url: impl AsRef<str>,
        on_batch: F,
    ) -> Result<IbbProfileReport>
    where
        F: FnMut(IbbProfileBatch) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        self.stream_profile_albums_with_session(input_url, Some(session), on_batch)
            .await
    }

    /// 按可选登录会话流式遍历 ImgBB 用户主页专辑。
    async fn stream_profile_albums_with_session<F, Fut>(
        &self,
        input_url: impl AsRef<str>,
        session: Option<&IbbLoginSession>,
        mut on_batch: F,
    ) -> Result<IbbProfileReport>
    where
        F: FnMut(IbbProfileBatch) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        let profile = IbbProfileUrl::parse(input_url.as_ref())?;
        info!(url = %profile.normalized_url, "开始遍历 ImgBB 用户主页专辑");

        let initial_html = self.fetch_profile_html(&profile, session).await?;
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
                .fetch_profile_albums_json(&profile, session, &auth_token, page, &current_seek)
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
    async fn fetch_profile_html(
        &self,
        profile: &IbbProfileUrl,
        session: Option<&IbbLoginSession>,
    ) -> Result<String> {
        let mut headers = browser_page_headers(&profile.origin)?;
        if let Some(session) = session {
            insert_header(&mut headers, COOKIE, &session.cookie_header)?;
        }
        let request = FetchRequest::get(profile.normalized_url.clone()).with_headers(headers);
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
        session: Option<&IbbLoginSession>,
        auth_token: &str,
        page: usize,
        seek: &str,
    ) -> Result<Value> {
        let body = profile.build_albums_json_body(auth_token, page, seek)?;
        let mut headers = browser_form_headers(&profile.normalized_url, &profile.origin)?;
        if let Some(session) = session {
            insert_header(&mut headers, COOKIE, &session.cookie_header)?;
        }
        let response = self
            .client
            .fetch(FetchRequest::post(profile.json_url.clone(), body).with_headers(headers))
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
