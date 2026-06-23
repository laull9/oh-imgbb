use anyhow::{Result, anyhow};
use scraper::{ElementRef, Html, Selector};
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

/// HtmlQuery 封装 HTML 文档和 CSS 选择器查询。
pub struct HtmlQuery {
    document: Html,
}

impl HtmlQuery {
    /// 从完整 HTML 文档创建查询器。
    pub fn document(html: &str) -> Self {
        Self {
            document: Html::parse_document(html),
        }
    }

    /// 从 HTML 片段创建查询器。
    pub fn fragment(html: &str) -> Self {
        Self {
            document: Html::parse_fragment(html),
        }
    }

    /// 查询所有匹配选择器的元素。
    pub fn select(&self, selector: &str) -> Result<Vec<HtmlElement<'_>>> {
        let selector = parse_selector(selector)?;
        Ok(self
            .document
            .select(&selector)
            .map(|element| HtmlElement { element })
            .collect())
    }

    /// 查询第一个匹配选择器的元素。
    pub fn first(&self, selector: &str) -> Result<Option<HtmlElement<'_>>> {
        let selector = parse_selector(selector)?;
        Ok(self
            .document
            .select(&selector)
            .next()
            .map(|element| HtmlElement { element }))
    }

    /// 查询第一个匹配元素的非空属性。
    pub fn first_attr(&self, selector: &str, attr: &str) -> Result<Option<String>> {
        Ok(self
            .first(selector)?
            .and_then(|element| element.trimmed_attr(attr)))
    }

    /// 查询所有匹配元素的非空属性。
    pub fn all_attrs(&self, selector: &str, attr: &str) -> Result<Vec<String>> {
        Ok(self
            .select(selector)?
            .into_iter()
            .filter_map(|element| element.trimmed_attr(attr))
            .collect())
    }

    /// 查询第一个匹配元素的归一化文本。
    pub fn first_text(&self, selector: &str) -> Result<Option<String>> {
        Ok(self
            .first(selector)?
            .map(|element| element.text())
            .filter(|text| !text.is_empty()))
    }

    /// 查询所有匹配元素的归一化文本。
    pub fn all_texts(&self, selector: &str) -> Result<Vec<String>> {
        Ok(self
            .select(selector)?
            .into_iter()
            .map(|element| element.text())
            .filter(|text| !text.is_empty())
            .collect())
    }
}

/// HtmlElement 封装单个 HTML 元素的常用读取操作。
pub struct HtmlElement<'a> {
    element: ElementRef<'a>,
}

impl<'a> HtmlElement<'a> {
    /// 读取元素的原始属性值。
    pub fn attr(&self, attr: &str) -> Option<&str> {
        self.element.value().attr(attr)
    }

    /// 读取元素的去空白非空属性值。
    pub fn trimmed_attr(&self, attr: &str) -> Option<String> {
        self.attr(attr)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    /// 读取元素的归一化文本内容。
    pub fn text(&self) -> String {
        normalize_text(&self.element.text().collect::<Vec<_>>().join(" "))
    }

    /// 查询子元素中的第一个匹配元素。
    pub fn first(&self, selector: &str) -> Result<Option<HtmlElement<'a>>> {
        let selector = parse_selector(selector)?;
        Ok(self
            .element
            .select(&selector)
            .next()
            .map(|element| HtmlElement { element }))
    }

    /// 查询子元素中的所有匹配元素。
    pub fn select(&self, selector: &str) -> Result<Vec<HtmlElement<'a>>> {
        let selector = parse_selector(selector)?;
        Ok(self
            .element
            .select(&selector)
            .map(|element| HtmlElement { element })
            .collect())
    }

    /// 查询子元素中第一个匹配元素的非空属性。
    pub fn first_attr(&self, selector: &str, attr: &str) -> Result<Option<String>> {
        Ok(self
            .first(selector)?
            .and_then(|element| element.trimmed_attr(attr)))
    }

    /// 查询子元素中所有匹配元素的非空属性。
    pub fn all_attrs(&self, selector: &str, attr: &str) -> Result<Vec<String>> {
        Ok(self
            .select(selector)?
            .into_iter()
            .filter_map(|element| element.trimmed_attr(attr))
            .collect())
    }

    /// 查询子元素中第一个匹配元素的归一化文本。
    pub fn first_text(&self, selector: &str) -> Result<Option<String>> {
        Ok(self
            .first(selector)?
            .map(|element| element.text())
            .filter(|text| !text.is_empty()))
    }

    /// 查询子元素中所有匹配元素的归一化文本。
    pub fn all_texts(&self, selector: &str) -> Result<Vec<String>> {
        Ok(self
            .select(selector)?
            .into_iter()
            .map(|element| element.text())
            .filter(|text| !text.is_empty())
            .collect())
    }
}

/// 从完整 HTML 文档查询第一个匹配元素的非空属性。
pub fn html_attr(html: &str, selector: &str, attr: &str) -> Result<Option<String>> {
    HtmlQuery::document(html).first_attr(selector, attr)
}

/// 从 HTML 片段查询第一个匹配元素的非空属性。
pub fn html_fragment_attr(html: &str, selector: &str, attr: &str) -> Result<Option<String>> {
    HtmlQuery::fragment(html).first_attr(selector, attr)
}

/// 从完整 HTML 文档查询所有匹配元素的非空属性。
pub fn html_attrs(html: &str, selector: &str, attr: &str) -> Result<Vec<String>> {
    HtmlQuery::document(html).all_attrs(selector, attr)
}

/// 从 HTML 片段查询所有匹配元素的非空属性。
pub fn html_fragment_attrs(html: &str, selector: &str, attr: &str) -> Result<Vec<String>> {
    HtmlQuery::fragment(html).all_attrs(selector, attr)
}

/// 从完整 HTML 文档查询第一个匹配元素的归一化文本。
pub fn html_text(html: &str, selector: &str) -> Result<Option<String>> {
    HtmlQuery::document(html).first_text(selector)
}

/// 从 HTML 片段查询第一个匹配元素的归一化文本。
pub fn html_fragment_text(html: &str, selector: &str) -> Result<Option<String>> {
    HtmlQuery::fragment(html).first_text(selector)
}

/// 从完整 HTML 文档查询所有匹配元素的归一化文本。
pub fn html_texts(html: &str, selector: &str) -> Result<Vec<String>> {
    HtmlQuery::document(html).all_texts(selector)
}

/// 从 HTML 片段查询所有匹配元素的归一化文本。
pub fn html_fragment_texts(html: &str, selector: &str) -> Result<Vec<String>> {
    HtmlQuery::fragment(html).all_texts(selector)
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
        let document = HtmlQuery::document(html);
        let title = self.extract_title(&document)?;
        let text = self.extract_text(&document)?;
        let links = self.extract_links(&document)?;

        Ok(ExtractResult { title, text, links })
    }

    /// 提取 HTML 标题。
    fn extract_title(&self, document: &HtmlQuery) -> Result<Option<String>> {
        document.first_text("title")
    }

    /// 提取页面可见文本。
    fn extract_text(&self, document: &HtmlQuery) -> Result<String> {
        Ok(document.first_text("body")?.unwrap_or_default())
    }

    /// 提取页面链接列表。
    fn extract_links(&self, document: &HtmlQuery) -> Result<Vec<Link>> {
        let links = document
            .select("a[href]")?
            .into_iter()
            .filter_map(|element| {
                let href = element.trimmed_attr("href")?;
                let text = element.text();
                debug!(href, text, "分析到跳转标签");
                Some(Link { text, href })
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

    /// 验证可以直接从文档字符串查询属性。
    #[test]
    fn html_attr_selects_trimmed_attribute_from_document() {
        let html = r#"<html><body><a href=" /docs "> Docs </a></body></html>"#;

        let href = html_attr(html, "a", "href").unwrap();

        assert_eq!(href, Some("/docs".to_string()));
    }

    /// 验证可以在元素上继续查询子选择器。
    #[test]
    fn html_query_selects_nested_attributes() {
        let html = r#"
            <div class="item" data-name=" demo ">
                <a href="/album/demo"><img src="/cover.jpg"></a>
                <span> first </span>
                <span> second </span>
            </div>
        "#;
        let query = HtmlQuery::fragment(html);
        let item = query.first(".item").unwrap().unwrap();

        assert_eq!(item.trimmed_attr("data-name"), Some("demo".to_string()));
        assert_eq!(
            item.first_attr("a > img", "src").unwrap(),
            Some("/cover.jpg".to_string())
        );
        assert_eq!(
            item.all_texts("span").unwrap(),
            vec!["first".to_string(), "second".to_string()]
        );
    }
}
