use reqwest::{StatusCode, header::HeaderMap};

use crate::analysis::extract::ExtractResult;
use crate::downloader::request::FetchResponse;
use crate::engine::task::TaskId;

/// PageBody 表示页面或资源响应的统一正文载体。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PageBody {
    Text(String),
    Binary(Vec<u8>),
}

impl PageBody {
    /// 返回文本正文引用。
    pub fn as_text(&self) -> Option<&str> {
        match self {
            PageBody::Text(text) => Some(text),
            PageBody::Binary(_) => None,
        }
    }
}

/// Page 是 Task 执行后的标准化中间数据表达。
#[derive(Clone, Debug)]
pub struct Page {
    pub task_id: TaskId,
    pub url: String,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub mime: Option<String>,
    pub body: PageBody,
    pub extract: Option<ExtractResult>,
}

impl Page {
    /// 从 HTTP 响应创建文本页面。
    pub fn from_response(task_id: TaskId, response: FetchResponse) -> Self {
        let mime = response
            .headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);

        Self {
            task_id,
            url: response.url,
            status: response.status,
            headers: response.headers,
            mime,
            body: PageBody::Text(response.body),
            extract: None,
        }
    }

    /// 设置页面提取结果。
    pub fn with_extract(mut self, extract: ExtractResult) -> Self {
        self.extract = Some(extract);
        self
    }

    /// 判断页面响应是否成功。
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloader::request::FetchResponse;

    /// 验证响应可以转换为页面。
    #[test]
    fn page_can_be_created_from_response() {
        let response = FetchResponse {
            url: "https://example.com".to_string(),
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: "hello".to_string(),
        };
        let page = Page::from_response(TaskId(1), response);

        assert_eq!(page.task_id, TaskId(1));
        assert_eq!(page.body.as_text(), Some("hello"));
        assert!(page.is_success());
    }
}
