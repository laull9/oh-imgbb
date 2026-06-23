use anyhow::Result;

use crate::analysis::extract::HtmlExtractor;
use crate::downloader::request::FetchResponse;
use crate::engine::page::Page;
use crate::engine::task::Task;

/// Parser 定义原始响应到标准 Page 的转换接口。
pub trait Parser: Send + Sync {
    /// 解析任务响应并返回页面。
    fn parse(&self, task: &Task, response: FetchResponse) -> Result<Page>;
}

/// HtmlParser 提供默认 HTML 文本解析实现。
#[derive(Clone, Debug, Default)]
pub struct HtmlParser {
    extractor: HtmlExtractor,
}

impl HtmlParser {
    /// 创建 HTML 解析器。
    pub fn new() -> Self {
        Self {
            extractor: HtmlExtractor::new(),
        }
    }
}

impl Parser for HtmlParser {
    /// 将响应转换为 Page 并尝试提取结构化内容。
    fn parse(&self, task: &Task, response: FetchResponse) -> Result<Page> {
        let body = response.body.clone();
        let mut page = Page::from_response(task.id, response);

        if !body.trim().is_empty() {
            page = page.with_extract(self.extractor.extract(&body)?);
        }

        Ok(page)
    }
}
