use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method, Proxy, Response};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tokio::fs;
use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::config::RequestConfig;
use crate::downloader::proxy::ProxyPool;
use crate::downloader::request::{
    DownloadResponse, FetchRequest, FetchResponse, HttpMethod, SavedDownload,
};
use crate::downloader::retry::RetryPolicy;
use crate::fake::{HeaderFakeStrategy, RandomHeaderFakeStrategy};
use crate::plugin::{PluginContext, PluginRegistry};

/// DEFAULT_MAX_CONCURRENT_REQUESTS 表示默认最大并发请求数。
pub const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 16;

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
        let path = path.as_ref();
        let response = self.download_request(request).await?;

        if !response.is_success() {
            return Err(anyhow!(
                "文件下载失败: {} {}",
                response.status,
                response.url
            ));
        }

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("创建下载目录失败: {}", parent.display()))?;
        }

        fs::write(path, &response.bytes)
            .await
            .with_context(|| format!("写入下载文件失败: {}", path.display()))?;

        Ok(SavedDownload {
            url: response.url,
            status: response.status,
            headers: response.headers,
            path: path.to_path_buf(),
            bytes_written: response.bytes.len(),
        })
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
        let client = self.client_for_request().await?;
        let method = to_reqwest_method(&request.method);
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
    async fn client_for_request(&self) -> Result<Client> {
        let Some(proxy_pool) = &self.proxy_pool else {
            return Ok(self.client.clone());
        };

        let Some(proxy_url) = proxy_pool.next_proxy()? else {
            return Ok(self.client.clone());
        };

        Client::builder()
            .proxy(Proxy::all(proxy_url)?)
            .build()
            .map_err(Into::into)
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
    use crate::fake::StaticHeaderFakeStrategy;
    use reqwest::header::USER_AGENT;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

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
        let path = std::env::temp_dir().join(format!(
            "llpha-download-{}.bin",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let saved = client.download_to_file(url, &path).await.unwrap();
        let bytes = fs::read(&path).await.unwrap();
        let _ = fs::remove_file(&path).await;

        assert_eq!(bytes, body);
        assert_eq!(saved.bytes_written, body.len());
        assert_eq!(saved.path, path);
    }

    /// 启动一次性 HTTP 测试服务。
    async fn serve_once(body: &'static [u8], content_disposition: Option<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 1024];
            let _ = socket.read(&mut buffer).await.unwrap();
            let mut response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n",
                body.len()
            );
            if let Some(content_disposition) = content_disposition {
                response.push_str(&format!("Content-Disposition: {content_disposition}\r\n"));
            }
            response.push_str("\r\n");
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.write_all(body).await.unwrap();
        });

        format!("http://{address}")
    }
}
