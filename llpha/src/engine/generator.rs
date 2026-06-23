use anyhow::Result;
use reqwest::Url;

use crate::engine::action::Action;
use crate::engine::page::Page;
use crate::engine::task::Task;

/// ActionGenerator 定义 Page 到 Action 的决策生成接口。
pub trait ActionGenerator: Send + Sync {
    /// 从任务和页面中生成下一步行为。
    fn generate(&self, task: &Task, page: &Page) -> Result<Vec<Action>>;
}

impl<F> ActionGenerator for F
where
    F: Fn(&Task, &Page) -> Result<Vec<Action>> + Send + Sync,
{
    /// 让闭包可以直接作为行为生成器使用。
    fn generate(&self, task: &Task, page: &Page) -> Result<Vec<Action>> {
        self(task, page)
    }
}

/// LinkActionGenerator 从 HTML 提取结果中生成链接跟踪行为。
#[derive(Clone, Debug)]
pub struct LinkActionGenerator {
    max_links_per_page: usize,
}

impl Default for LinkActionGenerator {
    /// 创建默认链接行为生成器。
    fn default() -> Self {
        Self::new()
    }
}

impl LinkActionGenerator {
    /// 创建链接行为生成器。
    pub fn new() -> Self {
        Self {
            max_links_per_page: 64,
        }
    }

    /// 设置单页最多生成的链接行为数量。
    pub fn with_max_links_per_page(mut self, max_links_per_page: usize) -> Self {
        self.max_links_per_page = max_links_per_page;
        self
    }
}

impl ActionGenerator for LinkActionGenerator {
    /// 从页面链接中生成跟踪行为。
    fn generate(&self, _task: &Task, page: &Page) -> Result<Vec<Action>> {
        let Some(extract) = &page.extract else {
            return Ok(vec![]);
        };

        let base_url = Url::parse(&page.url).ok();
        let actions = extract
            .links
            .iter()
            .take(self.max_links_per_page)
            .filter_map(|link| normalize_url(base_url.as_ref(), &link.href))
            .map(Action::follow_link)
            .collect();

        Ok(actions)
    }
}

/// 归一化链接为绝对 URL。
fn normalize_url(base_url: Option<&Url>, href: &str) -> Option<String> {
    if href.starts_with('#') || href.starts_with("javascript:") || href.starts_with("mailto:") {
        return None;
    }

    if let Ok(url) = Url::parse(href) {
        return Some(url.to_string());
    }

    base_url
        .and_then(|url| url.join(href).ok())
        .map(|url| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::extract::{ExtractResult, Link};
    use crate::engine::page::{Page, PageBody};
    use crate::engine::task::TaskId;
    use reqwest::StatusCode;
    use reqwest::header::HeaderMap;

    /// 验证链接生成器会解析相对链接。
    #[test]
    fn link_generator_resolves_relative_links() {
        let generator = LinkActionGenerator::new();
        let page = Page {
            task_id: TaskId(1),
            url: "https://example.com/a/index.html".to_string(),
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            mime: None,
            body: PageBody::Text(String::new()),
            extract: Some(ExtractResult {
                title: None,
                text: String::new(),
                links: vec![Link {
                    text: "Docs".to_string(),
                    href: "../docs".to_string(),
                }],
            }),
        };

        let actions = generator
            .generate(&Task::fetch("https://example.com"), &page)
            .unwrap();

        assert_eq!(
            actions[0].target,
            Some("https://example.com/docs".to_string())
        );
    }
}
