use anyhow::{Result, anyhow};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Link 表示页面中提取到的链接。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub text: String,
    pub href: String,
}

/// ExtractResult 表示从 HTML 页面中提取出的基础结构化内容。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtractResult {
    pub title: Option<String>,
    pub text: String,
    pub links: Vec<Link>,
}

/// HtmlExtractor 提供 HTML 文档的轻量结构化提取能力。
///
/// 当前实现聚焦 title、正文文本和链接提取，后续可以在不破坏调用方的前提下
/// 增加正文密度算法、元信息提取和站点特定规则。
#[derive(Clone, Debug, Default)]
pub struct HtmlExtractor;

impl HtmlExtractor {
    /// 创建 HTML 提取器。
    pub fn new() -> Self {
        Self
    }

    /// 从 HTML 字符串中提取基础结构化内容。
    pub fn extract(&self, html: &str) -> Result<ExtractResult> {
        let document = Html::parse_document(html);
        let title = self.extract_title(&document)?;
        let text = self.extract_text(&document)?;
        let links = self.extract_links(&document)?;

        Ok(ExtractResult { title, text, links })
    }

    /// 提取 HTML 标题。
    fn extract_title(&self, document: &Html) -> Result<Option<String>> {
        let selector = parse_selector("title")?;
        let title = document
            .select(&selector)
            .next()
            .map(|element| normalize_text(&element.text().collect::<Vec<_>>().join(" ")))
            .filter(|text| !text.is_empty());

        Ok(title)
    }

    /// 提取页面可见文本。
    fn extract_text(&self, document: &Html) -> Result<String> {
        let selector = parse_selector("body")?;
        let text = document
            .select(&selector)
            .next()
            .map(|element| normalize_text(&element.text().collect::<Vec<_>>().join(" ")))
            .unwrap_or_default();

        Ok(text)
    }

    /// 提取页面链接列表。
    fn extract_links(&self, document: &Html) -> Result<Vec<Link>> {
        let selector = parse_selector("a[href]")?;
        let links = document
            .select(&selector)
            .filter_map(|element| {
                let href = element.value().attr("href")?.trim();
                if href.is_empty() {
                    return None;
                }

                let text = normalize_text(&element.text().collect::<Vec<_>>().join(" "));
                debug!(href, text, "分析到跳转标签");
                Some(Link {
                    text,
                    href: href.to_string(),
                })
            })
            .collect();

        Ok(links)
    }
}

/// 解析 CSS 选择器并转换错误类型。
fn parse_selector(selector: &str) -> Result<Selector> {
    Selector::parse(selector).map_err(|err| anyhow!("CSS选择器解析失败 {selector}: {err}"))
}

/// 归一化文本中的多余空白。
fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// 验证提取器可以提取标题、正文和链接。
    fn html_extractor_extracts_title_text_and_links() {
        let html = r#"
            <html>
                <head><title> Llpha Demo </title></head>
                <body>
                    <h1>Hello</h1>
                    <a href="/docs"> Docs </a>
                </body>
            </html>
        "#;

        let result = HtmlExtractor::new().extract(html).unwrap();

        assert_eq!(result.title, Some("Llpha Demo".to_string()));
        assert!(result.text.contains("Hello"));
        assert_eq!(
            result.links,
            vec![Link {
                text: "Docs".to_string(),
                href: "/docs".to_string(),
            }]
        );
    }
}
