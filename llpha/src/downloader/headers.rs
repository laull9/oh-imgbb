//! headers 模块提供常用 HTTP 请求头构造能力。

use anyhow::{Context, Result};
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, ORIGIN, REFERER,
    USER_AGENT,
};

/// DEFAULT_BROWSER_USER_AGENT 保存默认桌面浏览器 User-Agent。
pub const DEFAULT_BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";

const DEFAULT_ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9";
const HTML_ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
const JSON_ACCEPT: &str = "application/json, text/javascript, */*; q=0.01";
const IMAGE_ACCEPT: &str = "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";
const FORM_CONTENT_TYPE: &str = "application/x-www-form-urlencoded; charset=UTF-8";

/// 构造浏览器页面请求头。
pub fn browser_page_headers(referer: &str) -> Result<HeaderMap> {
    let mut headers = base_browser_headers(referer)?;
    insert_header(&mut headers, ACCEPT, HTML_ACCEPT)?;
    Ok(headers)
}

/// 构造浏览器表单接口请求头。
pub fn browser_form_headers(referer: &str, origin: &str) -> Result<HeaderMap> {
    let mut headers = base_browser_headers(referer)?;
    insert_header(&mut headers, ACCEPT, JSON_ACCEPT)?;
    insert_header(&mut headers, CONTENT_TYPE, FORM_CONTENT_TYPE)?;
    insert_header(&mut headers, ORIGIN, origin)?;
    insert_header(
        &mut headers,
        HeaderName::from_static("x-requested-with"),
        "XMLHttpRequest",
    )?;
    Ok(headers)
}

/// 构造图片下载请求头。
pub fn image_download_headers(referer: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, USER_AGENT, DEFAULT_BROWSER_USER_AGENT)?;
    insert_header(&mut headers, ACCEPT_LANGUAGE, DEFAULT_ACCEPT_LANGUAGE)?;
    insert_header(&mut headers, REFERER, referer)?;
    insert_header(&mut headers, ACCEPT, IMAGE_ACCEPT)?;
    Ok(headers)
}

/// 插入字符串请求头并转换错误类型。
pub fn insert_header(headers: &mut HeaderMap, name: HeaderName, value: &str) -> Result<()> {
    let value = HeaderValue::from_str(value)
        .with_context(|| format!("解析请求头值失败: {}", name.as_str()))?;
    headers.insert(name, value);
    Ok(())
}

/// 构造基础浏览器请求头。
fn base_browser_headers(referer: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, USER_AGENT, DEFAULT_BROWSER_USER_AGENT)?;
    insert_header(&mut headers, ACCEPT_LANGUAGE, DEFAULT_ACCEPT_LANGUAGE)?;
    insert_header(&mut headers, REFERER, referer)?;
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证浏览器页面请求头包含 HTML Accept。
    #[test]
    fn browser_page_headers_include_html_accept() {
        let headers = browser_page_headers("https://example.com").unwrap();

        assert_eq!(headers.get(ACCEPT).unwrap(), HTML_ACCEPT);
        assert!(headers.contains_key(USER_AGENT));
        assert!(headers.contains_key(REFERER));
    }

    /// 验证浏览器表单请求头包含 Ajax 字段。
    #[test]
    fn browser_form_headers_include_ajax_headers() {
        let headers =
            browser_form_headers("https://example.com/page", "https://example.com").unwrap();

        assert_eq!(headers.get(ACCEPT).unwrap(), JSON_ACCEPT);
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), FORM_CONTENT_TYPE);
        assert_eq!(headers.get(ORIGIN).unwrap(), "https://example.com");
        assert_eq!(
            headers
                .get(HeaderName::from_static("x-requested-with"))
                .unwrap(),
            "XMLHttpRequest"
        );
    }

    /// 验证图片下载请求头包含图片 Accept。
    #[test]
    fn image_download_headers_include_image_accept() {
        let headers = image_download_headers("https://example.com/album").unwrap();

        assert_eq!(headers.get(ACCEPT).unwrap(), IMAGE_ACCEPT);
        assert_eq!(headers.get(REFERER).unwrap(), "https://example.com/album");
    }
}
