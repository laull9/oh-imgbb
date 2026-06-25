use reqwest::{
    StatusCode,
    header::{CONTENT_DISPOSITION, HeaderMap},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// HttpMethod 表示一次请求使用的 HTTP 方法。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
}

/// RequestOptions 保存单次请求的可选配置。
#[derive(Clone, Debug)]
pub struct RequestOptions {
    pub timeout: Option<Duration>,
    pub headers: HeaderMap,
    pub follow_redirects: bool,
}

impl Default for RequestOptions {
    /// 创建默认请求配置。
    fn default() -> Self {
        Self {
            timeout: None,
            headers: HeaderMap::new(),
            follow_redirects: true,
        }
    }
}

impl RequestOptions {
    /// 设置单次请求超时时间。
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 设置单次请求头集合。
    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// 禁用单次请求自动跟随跳转。
    pub fn without_redirects(mut self) -> Self {
        self.follow_redirects = false;
        self
    }
}

/// FetchRequest 描述一次即将发起的网络请求。
#[derive(Clone, Debug)]
pub struct FetchRequest {
    pub method: HttpMethod,
    pub url: String,
    pub body: Option<String>,
    pub options: RequestOptions,
}

impl FetchRequest {
    /// 创建 GET 请求。
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Get,
            url: url.into(),
            body: None,
            options: RequestOptions::default(),
        }
    }

    /// 创建 POST 请求。
    pub fn post(url: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Post,
            url: url.into(),
            body: Some(body.into()),
            options: RequestOptions::default(),
        }
    }

    /// 设置请求配置。
    pub fn with_options(mut self, options: RequestOptions) -> Self {
        self.options = options;
        self
    }

    /// 设置请求头集合。
    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.options.headers = headers;
        self
    }

    /// 设置单次请求超时时间。
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = Some(timeout);
        self
    }

    /// 禁用本次请求自动跟随跳转。
    pub fn without_redirects(mut self) -> Self {
        self.options.follow_redirects = false;
        self
    }
}

impl From<&str> for FetchRequest {
    /// 将 URL 字符串转换为默认 GET 请求。
    fn from(url: &str) -> Self {
        Self::get(url)
    }
}

impl From<String> for FetchRequest {
    /// 将 URL 字符串转换为默认 GET 请求。
    fn from(url: String) -> Self {
        Self::get(url)
    }
}

impl From<&String> for FetchRequest {
    /// 将 URL 字符串引用转换为默认 GET 请求。
    fn from(url: &String) -> Self {
        Self::get(url.clone())
    }
}

/// FetchResponse 保存网络请求返回的状态、头和正文。
#[derive(Clone, Debug)]
pub struct FetchResponse {
    pub url: String,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

impl FetchResponse {
    /// 判断响应状态是否为 2xx。
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}

/// DownloadResponse 保存文件下载返回的状态、头和二进制内容。
#[derive(Clone, Debug)]
pub struct DownloadResponse {
    pub url: String,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub bytes: Vec<u8>,
}

impl DownloadResponse {
    /// 判断响应状态是否为 2xx。
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// 返回下载内容字节数。
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// 判断下载内容是否为空。
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// 从 Content-Disposition 中推断文件名。
    pub fn suggested_file_name(&self) -> Option<String> {
        let value = self.headers.get(CONTENT_DISPOSITION)?.to_str().ok()?;
        parse_content_disposition_file_name(value)
    }
}

/// SavedDownload 保存一次落盘下载的结果。
#[derive(Clone, Debug)]
pub struct SavedDownload {
    pub url: String,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub path: PathBuf,
    pub bytes_written: usize,
}

impl SavedDownload {
    /// 判断落盘下载状态是否为 2xx。
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}

/// 解析 Content-Disposition 中的 filename 参数。
fn parse_content_disposition_file_name(value: &str) -> Option<String> {
    value.split(';').find_map(|part| {
        let part = part.trim();
        let file_name = part.strip_prefix("filename=")?;
        Some(file_name.trim_matches('"').to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    /// 验证请求配置可以设置超时时间。
    fn request_options_can_set_timeout() {
        let options = RequestOptions::default().with_timeout(Duration::from_secs(3));

        assert_eq!(options.timeout, Some(Duration::from_secs(3)));
        assert!(options.follow_redirects);
    }

    #[test]
    /// 验证可以构建 GET 请求。
    fn fetch_request_can_build_get() {
        let request = FetchRequest::get("https://example.com");

        assert_eq!(request.method, HttpMethod::Get);
        assert_eq!(request.url, "https://example.com");
        assert!(request.body.is_none());
    }

    /// 验证请求可以禁用自动跳转。
    #[test]
    fn fetch_request_can_disable_redirects() {
        let request = FetchRequest::get("https://example.com").without_redirects();

        assert!(!request.options.follow_redirects);
    }

    /// 验证 URL 字符串可以直接转换为 GET 请求。
    #[test]
    fn string_converts_to_fetch_request() {
        let request = FetchRequest::from("https://example.com");

        assert_eq!(request.method, HttpMethod::Get);
        assert_eq!(request.url, "https://example.com");
    }

    /// 验证下载响应可以解析建议文件名。
    #[test]
    fn download_response_can_parse_suggested_file_name() {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"report.pdf\""),
        );
        let response = DownloadResponse {
            url: "https://example.com/report.pdf".to_string(),
            status: StatusCode::OK,
            headers,
            bytes: vec![1, 2, 3],
        };

        assert_eq!(
            response.suggested_file_name(),
            Some("report.pdf".to_string())
        );
        assert_eq!(response.len(), 3);
    }
}
