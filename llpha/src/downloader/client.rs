use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method, Proxy, Response, redirect::Policy};
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;
use tokio::time::sleep;

#[path = "client_file_download.rs"]
mod client_file_download;

use crate::config::RequestConfig;
use crate::downloader::progress::{DownloadProgress, download_progress_callback};
use crate::downloader::proxy::ProxyPool;
use crate::downloader::request::{
    DownloadResponse, FetchRequest, FetchResponse, HttpMethod, SavedDownload,
};
use crate::downloader::retry::RetryPolicy;
use crate::fake::{HeaderFakeStrategy, RandomHeaderFakeStrategy};
use crate::plugin::{PluginContext, PluginRegistry};

/// DEFAULT_MAX_CONCURRENT_REQUESTS 表示默认最大并发请求数。
pub const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 16;

/// DEFAULT_PARALLEL_DOWNLOAD_THRESHOLD 表示启用分片下载的默认文件大小。
pub const DEFAULT_PARALLEL_DOWNLOAD_THRESHOLD: u64 = 10 * 1024 * 1024;

/// DEFAULT_DOWNLOAD_PART_SIZE 表示默认分片大小。
pub(super) const DEFAULT_DOWNLOAD_PART_SIZE: u64 = 5 * 1024 * 1024;

/// DEFAULT_MAX_DOWNLOAD_PARTS 表示单文件默认最大分片并发数。
pub(super) const DEFAULT_MAX_DOWNLOAD_PARTS: usize = 8;

/// GLOBAL_CLIENT 保存按需初始化的默认客户端。
static GLOBAL_CLIENT: OnceLock<Arc<LlphaClient>> = OnceLock::new();

/// LlphaClientBuilder 构建带重试、代理和插件能力的客户端。
pub struct LlphaClientBuilder {
    retry_policy: RetryPolicy,
    max_concurrent_requests: usize,
    proxy_pool: Option<Arc<dyn ProxyPool>>,
    fake_strategy: Option<Arc<dyn HeaderFakeStrategy>>,
    plugin_registry: PluginRegistry,
}

impl Default for LlphaClientBuilder {
    /// 创建默认客户端构建器。
    fn default() -> Self {
        Self {
            retry_policy: RetryPolicy::default(),
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            proxy_pool: None,
            fake_strategy: Some(Arc::new(RandomHeaderFakeStrategy::default())),
            plugin_registry: PluginRegistry::new(),
        }
    }
}

impl LlphaClientBuilder {
    /// 创建新的客户端构建器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置重试策略。
    pub fn retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    /// 设置最大并发请求数。
    pub fn max_concurrent_requests(mut self, max_concurrent_requests: usize) -> Self {
        self.max_concurrent_requests = max_concurrent_requests.max(1);
        self
    }

    /// 设置代理池。
    pub fn proxy_pool<P>(mut self, proxy_pool: P) -> Self
    where
        P: ProxyPool + 'static,
    {
        self.proxy_pool = Some(Arc::new(proxy_pool));
        self
    }

    /// 设置请求头伪装策略。
    pub fn fake_strategy<F>(mut self, fake_strategy: F) -> Self
    where
        F: HeaderFakeStrategy + 'static,
    {
        self.fake_strategy = Some(Arc::new(fake_strategy));
        self
    }

    /// 关闭默认请求头伪装策略。
    pub fn without_fake(mut self) -> Self {
        self.fake_strategy = None;
        self
    }

    /// 设置插件注册表。
    pub fn plugin_registry(mut self, plugin_registry: PluginRegistry) -> Self {
        self.plugin_registry = plugin_registry;
        self
    }

    /// 构建客户端实例。
    pub fn build(self) -> Result<LlphaClient> {
        let client = Client::builder().build()?;

        Ok(LlphaClient {
            client,
            retry_policy: self.retry_policy,
            max_concurrent_requests: self.max_concurrent_requests,
            request_limiter: Arc::new(Semaphore::new(self.max_concurrent_requests)),
            proxy_pool: self.proxy_pool,
            fake_strategy: self.fake_strategy,
            plugin_registry: self.plugin_registry,
            plugin_context: PluginContext {
                name: "llpha".to_string(),
            },
        })
    }
}

/// LlphaClient 是发起抓取请求的核心入口。
pub struct LlphaClient {
    client: Client,
    retry_policy: RetryPolicy,
    max_concurrent_requests: usize,
    request_limiter: Arc<Semaphore>,
    proxy_pool: Option<Arc<dyn ProxyPool>>,
    fake_strategy: Option<Arc<dyn HeaderFakeStrategy>>,
    plugin_registry: PluginRegistry,
    plugin_context: PluginContext,
}

impl LlphaClient {
    /// 创建默认客户端。
    pub fn new() -> Result<Self> {
        LlphaClientBuilder::new().build()
    }

    /// 返回按需初始化的全局客户端。
    pub fn global() -> Arc<Self> {
        GLOBAL_CLIENT
            .get_or_init(|| Arc::new(Self::new().expect("创建默认 LlphaClient 失败")))
            .clone()
    }

    /// 使用请求配置初始化全局客户端。
    pub fn init_global(request_config: &RequestConfig) -> Result<Arc<Self>> {
        let client = Arc::new(Self::from_request_config(request_config)?);
        GLOBAL_CLIENT
            .set(client.clone())
            .map_err(|_| anyhow!("全局 LlphaClient 已初始化，请在首次使用前加载配置"))?;

        Ok(client)
    }

    /// 按请求配置创建客户端。
    pub fn from_request_config(request_config: &RequestConfig) -> Result<Self> {
        let retry_policy = RetryPolicy::new(
            request_config
                .max_retries
                .min(crate::downloader::retry::DEFAULT_MAX_RETRIES),
        )
        .with_base_delay(std::time::Duration::from_millis(
            request_config.retry_base_delay_ms,
        ))
        .with_max_delay(std::time::Duration::from_millis(
            request_config.retry_max_delay_ms,
        ))
        .with_jitter(request_config.retry_jitter);

        Self::builder()
            .retry_policy(retry_policy)
            .max_concurrent_requests(request_config.max_concurrent_requests)
            .build()
    }

    /// 创建客户端构建器。
    pub fn builder() -> LlphaClientBuilder {
        LlphaClientBuilder::new()
    }

    /// 返回最大并发请求数。
    pub fn max_concurrent_requests(&self) -> usize {
        self.max_concurrent_requests
    }

    /// 以 GET 方法拉取指定 URL。
    pub async fn get(&self, url: impl Into<String>) -> Result<FetchResponse> {
        self.fetch(FetchRequest::get(url)).await
    }

    /// 以 POST 方法提交文本正文。
    pub async fn post(
        &self,
        url: impl Into<String>,
        body: impl Into<String>,
    ) -> Result<FetchResponse> {
        self.fetch(FetchRequest::post(url, body)).await
    }

    /// 下载指定 URL 并在内存中返回二进制内容。
    pub async fn download(&self, url: impl Into<String>) -> Result<DownloadResponse> {
        self.download_request(FetchRequest::get(url)).await
    }

    /// 下载指定 URL 并保存到文件。
    pub async fn download_to_file(
        &self,
        url: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<SavedDownload> {
        self.download_file(url, path).await
    }

    /// 下载指定 URL 并保存到文件。
    pub async fn download_file(
        &self,
        url: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<SavedDownload> {
        self.download_request_to_file(FetchRequest::get(url), path)
            .await
    }

    /// 下载指定 URL 并通过异步回调报告进度。
    pub async fn download_file_with_progress<F, Fut>(
        &self,
        url: impl Into<String>,
        path: impl AsRef<Path>,
        progress: F,
    ) -> Result<SavedDownload>
    where
        F: Fn(DownloadProgress) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.download_request_to_file_with_progress(FetchRequest::get(url), path, progress)
            .await
    }

    /// 执行一次可配置请求并应用插件、代理和重试策略。
    pub async fn fetch(&self, request: impl Into<FetchRequest>) -> Result<FetchResponse> {
        let request = request.into();
        let request = match &self.fake_strategy {
            Some(fake_strategy) => fake_strategy.apply(request)?,
            None => request,
        };
        let request = self
            .plugin_registry
            .apply_before_request(&self.plugin_context, request)?;

        let response = self.fetch_with_retry(&request).await?;

        self.plugin_registry
            .apply_after_response(&self.plugin_context, response)
    }

    /// 执行一次可配置下载并返回二进制内容。
    pub async fn download_request(
        &self,
        request: impl Into<FetchRequest>,
    ) -> Result<DownloadResponse> {
        let request = self.prepare_request(request.into())?;

        self.download_with_retry(&request).await
    }

    /// 执行一次可配置下载并保存二进制内容。
    pub async fn download_request_to_file(
        &self,
        request: impl Into<FetchRequest>,
        path: impl AsRef<Path>,
    ) -> Result<SavedDownload> {
        let request = self.prepare_request(request.into())?;

        client_file_download::download_prepared_request_to_file(self, &request, path.as_ref(), None)
            .await
    }

    /// 执行一次可配置下载并通过异步回调报告进度。
    pub async fn download_request_to_file_with_progress<F, Fut>(
        &self,
        request: impl Into<FetchRequest>,
        path: impl AsRef<Path>,
        progress: F,
    ) -> Result<SavedDownload>
    where
        F: Fn(DownloadProgress) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let request = self.prepare_request(request.into())?;
        let callback = Some(download_progress_callback(progress));

        client_file_download::download_prepared_request_to_file(
            self,
            &request,
            path.as_ref(),
            callback,
        )
        .await
    }

    /// 应用伪装策略和请求前插件。
    fn prepare_request(&self, request: FetchRequest) -> Result<FetchRequest> {
        let request = match &self.fake_strategy {
            Some(fake_strategy) => fake_strategy.apply(request)?,
            None => request,
        };

        self.plugin_registry
            .apply_before_request(&self.plugin_context, request)
    }

    /// 使用重试策略执行请求。
    async fn fetch_with_retry(&self, request: &FetchRequest) -> Result<FetchResponse> {
        let mut last_error = None;

        for attempt in 0..=self.retry_policy.max_retries {
            match self.send_once(request).await {
                Ok(response) if !self.retry_policy.should_retry_status(response.status) => {
                    return Ok(response);
                }
                Ok(response) => {
                    last_error = Some(anyhow!("请求返回可重试状态码: {}", response.status));
                    if attempt == self.retry_policy.max_retries {
                        return Ok(response);
                    }
                }
                Err(err) => {
                    last_error = Some(err);
                    if attempt == self.retry_policy.max_retries {
                        break;
                    }
                }
            }

            sleep(self.retry_policy.delay_for_attempt(attempt)).await;
        }

        Err(last_error.unwrap_or_else(|| anyhow!("请求失败且没有返回错误详情")))
    }

    /// 使用重试策略执行文件下载。
    async fn download_with_retry(&self, request: &FetchRequest) -> Result<DownloadResponse> {
        let mut last_error = None;

        for attempt in 0..=self.retry_policy.max_retries {
            match self.send_download_once(request).await {
                Ok(response) if !self.retry_policy.should_retry_status(response.status) => {
                    return Ok(response);
                }
                Ok(response) => {
                    last_error = Some(anyhow!("下载返回可重试状态码: {}", response.status));
                    if attempt == self.retry_policy.max_retries {
                        return Ok(response);
                    }
                }
                Err(err) => {
                    last_error = Some(err);
                    if attempt == self.retry_policy.max_retries {
                        break;
                    }
                }
            }

            sleep(self.retry_policy.delay_for_attempt(attempt)).await;
        }

        Err(last_error.unwrap_or_else(|| anyhow!("下载失败且没有返回错误详情")))
    }

    /// 发送一次请求且不做重试。
    async fn send_once(&self, request: &FetchRequest) -> Result<FetchResponse> {
        let _permit = self
            .request_limiter
            .clone()
            .acquire_owned()
            .await
            .context("获取请求限流额度失败")?;
        let response = self.send_raw_once(request).await?;
        let url = response.url().to_string();
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.text().await?;

        Ok(FetchResponse {
            url,
            status,
            headers,
            body,
        })
    }

    /// 发送一次下载请求且不做重试。
    async fn send_download_once(&self, request: &FetchRequest) -> Result<DownloadResponse> {
        let _permit = self
            .request_limiter
            .clone()
            .acquire_owned()
            .await
            .context("获取下载限流额度失败")?;
        let response = self.send_raw_once(request).await?;
        let url = response.url().to_string();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.bytes().await?.to_vec();

        Ok(DownloadResponse {
            url,
            status,
            headers,
            bytes,
        })
    }

    /// 发送一次原始 reqwest 请求。
    async fn send_raw_once(&self, request: &FetchRequest) -> Result<Response> {
        self.send_raw_with_method_once(request, to_reqwest_method(&request.method))
            .await
    }

    /// 使用指定 HTTP 方法发送一次原始 reqwest 请求。
    async fn send_raw_with_method_once(
        &self,
        request: &FetchRequest,
        method: Method,
    ) -> Result<Response> {
        let client = self.client_for_request(request).await?;
        let mut builder = client
            .request(method, &request.url)
            .headers(request.options.headers.clone());

        if let Some(timeout) = request.options.timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }

        builder.send().await.map_err(Into::into)
    }

    /// 根据代理池为当前请求创建 reqwest 客户端。
    async fn client_for_request(&self, request: &FetchRequest) -> Result<Client> {
        let Some(proxy_pool) = &self.proxy_pool else {
            if request.options.follow_redirects {
                return Ok(self.client.clone());
            }

            return Client::builder()
                .redirect(Policy::none())
                .build()
                .map_err(Into::into);
        };

        let Some(proxy_url) = proxy_pool.next_proxy()? else {
            if request.options.follow_redirects {
                return Ok(self.client.clone());
            }

            return Client::builder()
                .redirect(Policy::none())
                .build()
                .map_err(Into::into);
        };

        let mut builder = Client::builder().proxy(Proxy::all(proxy_url)?);
        if !request.options.follow_redirects {
            builder = builder.redirect(Policy::none());
        }

        builder.build().map_err(Into::into)
    }
}

/// 转换内部 HTTP 方法为 reqwest 方法。
fn to_reqwest_method(method: &HttpMethod) -> Method {
    match method {
        HttpMethod::Get => Method::GET,
        HttpMethod::Post => Method::POST,
        HttpMethod::Put => Method::PUT,
        HttpMethod::Patch => Method::PATCH,
        HttpMethod::Delete => Method::DELETE,
        HttpMethod::Head => Method::HEAD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloader::progress::DownloadProgressEvent;
    use crate::fake::StaticHeaderFakeStrategy;
    use reqwest::header::USER_AGENT;
    use tokio::fs;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    #[test]
    /// 验证内部 HTTP 方法可以转换为 reqwest 方法。
    fn method_mapping_supports_common_methods() {
        assert_eq!(to_reqwest_method(&HttpMethod::Get), Method::GET);
        assert_eq!(to_reqwest_method(&HttpMethod::Post), Method::POST);
        assert_eq!(to_reqwest_method(&HttpMethod::Delete), Method::DELETE);
    }

    /// 验证客户端构建器可以配置请求伪装策略。
    #[test]
    fn builder_accepts_fake_strategy() {
        let client = LlphaClient::builder()
            .fake_strategy(StaticHeaderFakeStrategy::default())
            .build()
            .unwrap();

        let request = client
            .fake_strategy
            .as_ref()
            .unwrap()
            .apply(FetchRequest::get("https://example.com"))
            .unwrap();

        assert!(request.options.headers.contains_key(USER_AGENT));
    }

    /// 验证默认客户端会启用请求头伪装。
    #[test]
    fn default_client_enables_fake_strategy() {
        let client = LlphaClient::new().unwrap();

        assert!(client.fake_strategy.is_some());
        assert_eq!(
            client.max_concurrent_requests(),
            DEFAULT_MAX_CONCURRENT_REQUESTS
        );
    }

    /// 验证可以显式关闭默认请求头伪装。
    #[test]
    fn builder_can_disable_fake_strategy() {
        let client = LlphaClient::builder().without_fake().build().unwrap();

        assert!(client.fake_strategy.is_none());
    }

    /// 验证客户端构建器可以配置请求限流。
    #[test]
    fn builder_accepts_max_concurrent_requests() {
        let client = LlphaClient::builder()
            .max_concurrent_requests(8)
            .build()
            .unwrap();

        assert_eq!(client.max_concurrent_requests(), 8);
    }

    /// 验证客户端抓取接口可以直接接收 URL 字符串。
    #[tokio::test]
    async fn client_fetch_accepts_url_string() {
        let body = b"hello page";
        let url = serve_once(body, None).await;
        let client = LlphaClient::new().unwrap();

        let response = client.fetch(url).await.unwrap();

        assert_eq!(response.body, "hello page");
    }

    /// 验证禁用跳转时可以读取原始 302 响应。
    #[tokio::test]
    async fn client_fetch_can_disable_redirects() {
        let url = serve_redirect_once("/next").await;
        let client = LlphaClient::new().unwrap();

        let response = client
            .fetch(FetchRequest::get(url).without_redirects())
            .await
            .unwrap();

        assert_eq!(response.status, reqwest::StatusCode::FOUND);
        assert_eq!(
            response.headers.get(reqwest::header::LOCATION).unwrap(),
            "/next"
        );
    }

    /// 验证客户端下载接口可以读取二进制内容。
    #[tokio::test]
    async fn client_download_reads_bytes() {
        let body = b"hello file";
        let url = serve_once(body, None).await;
        let client = LlphaClient::new().unwrap();

        let response = client.download(url).await.unwrap();

        assert_eq!(response.bytes, body);
        assert_eq!(response.len(), body.len());
    }

    /// 验证客户端下载接口可以保存文件。
    #[tokio::test]
    async fn client_download_to_file_writes_bytes() {
        let body = b"saved file";
        let url = serve_once(body, Some("attachment; filename=\"saved.bin\"")).await;
        let client = LlphaClient::new().unwrap();
        let path = temp_download_path("saved");

        let saved = client.download_to_file(url, &path).await.unwrap();
        let bytes = fs::read(&path).await.unwrap();
        let _ = fs::remove_file(&path).await;

        assert_eq!(bytes, body);
        assert_eq!(saved.bytes_written, body.len());
        assert_eq!(saved.path, path);
    }

    /// 验证落盘下载会报告异步进度。
    #[tokio::test]
    async fn client_download_to_file_reports_async_progress() {
        let body = b"progress file";
        let url = serve_once(body, None).await;
        let client = LlphaClient::new().unwrap();
        let path = temp_download_path("progress");
        let events = Arc::new(Mutex::new(Vec::new()));
        let progress_events = events.clone();

        let saved = client
            .download_file_with_progress(url, &path, move |progress| {
                let progress_events = progress_events.clone();
                async move {
                    progress_events.lock().await.push(progress);
                }
            })
            .await
            .unwrap();

        let progresses = events.lock().await.clone();
        let _ = fs::remove_file(&path).await;

        assert_eq!(saved.bytes_written, body.len());
        assert!(
            progresses
                .iter()
                .any(|progress| progress.event == DownloadProgressEvent::Started)
        );
        assert!(
            progresses
                .iter()
                .any(|progress| progress.event == DownloadProgressEvent::Advanced)
        );
        assert!(progresses.iter().any(|progress| {
            progress.event == DownloadProgressEvent::Finished
                && progress.downloaded_bytes == body.len() as u64
        }));
    }

    /// 验证大文件会使用 Range 分片下载并自动合并。
    #[tokio::test]
    async fn client_download_to_file_merges_parallel_parts() {
        let size = DEFAULT_PARALLEL_DOWNLOAD_THRESHOLD as usize + 1024;
        let body = Arc::new(
            (0..size)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>(),
        );
        let url = serve_range_file(body.clone()).await;
        let client = LlphaClient::builder()
            .max_concurrent_requests(4)
            .build()
            .unwrap();
        let path = temp_download_path("parallel");
        let events = Arc::new(Mutex::new(Vec::new()));
        let progress_events = events.clone();

        let saved = client
            .download_file_with_progress(url, &path, move |progress| {
                let progress_events = progress_events.clone();
                async move {
                    progress_events.lock().await.push(progress.event);
                }
            })
            .await
            .unwrap();
        let bytes = fs::read(&path).await.unwrap();
        let events = events.lock().await.clone();
        let _ = fs::remove_file(&path).await;

        assert_eq!(bytes, body.as_slice());
        assert_eq!(saved.bytes_written, size);
        assert!(
            events
                .iter()
                .filter(|event| matches!(event, DownloadProgressEvent::PartStarted { .. }))
                .count()
                > 1
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, DownloadProgressEvent::PartFinished { .. }))
        );
    }

    /// 生成临时下载文件路径。
    fn temp_download_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "llpha-download-{name}-{}.bin",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    /// 启动一次性跳转响应服务。
    async fn serve_redirect_once(location: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 1024];
            let _ = socket.read(&mut buffer).await.unwrap();
            let response =
                format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\n\r\n");
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        format!("http://{address}")
    }

    /// 启动 HTTP 测试服务。
    async fn serve_once(body: &'static [u8], content_disposition: Option<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = [0_u8; 1024];
                let read_size = socket.read(&mut buffer).await.unwrap();
                let request = String::from_utf8_lossy(&buffer[..read_size]);
                let is_head = request.starts_with("HEAD ");
                let mut response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nAccept-Ranges: bytes\r\nConnection: close\r\n",
                    body.len()
                );
                if let Some(content_disposition) = content_disposition {
                    response.push_str(&format!("Content-Disposition: {content_disposition}\r\n"));
                }
                response.push_str("\r\n");
                socket.write_all(response.as_bytes()).await.unwrap();
                if !is_head {
                    socket.write_all(body).await.unwrap();
                }
            }
        });

        format!("http://{address}")
    }

    /// 启动支持 Range 的 HTTP 测试服务。
    async fn serve_range_file(body: Arc<Vec<u8>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let body = body.clone();
                tokio::spawn(async move {
                    serve_range_connection(socket, body).await;
                });
            }
        });

        format!("http://{address}")
    }

    /// 响应一次支持 Range 的 HTTP 请求。
    async fn serve_range_connection(mut socket: tokio::net::TcpStream, body: Arc<Vec<u8>>) {
        let mut buffer = [0_u8; 2048];
        let read_size = socket.read(&mut buffer).await.unwrap();
        let request = String::from_utf8_lossy(&buffer[..read_size]);
        let is_head = request.starts_with("HEAD ");
        let range = request.lines().find_map(parse_range_header);

        if let Some((start, end)) = range {
            let end = end.min(body.len().saturating_sub(1));
            let content = &body[start..=end];
            let response = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                content.len(),
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.write_all(content).await.unwrap();
            return;
        }

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        if !is_head {
            socket.write_all(&body).await.unwrap();
        }
    }

    /// 解析 HTTP Range 请求头。
    fn parse_range_header(line: &str) -> Option<(usize, usize)> {
        let value = line
            .strip_prefix("Range:")
            .or_else(|| line.strip_prefix("range:"))?
            .trim()
            .strip_prefix("bytes=")?;
        let (start, end) = value.split_once('-')?;

        Some((start.parse().ok()?, end.parse().ok()?))
    }
}
