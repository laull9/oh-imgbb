//! analysis 模块提供页面内容提取和后续分析能力。

pub mod extract;
pub mod json;

pub use extract::{
    ExtractResult, HtmlElement, HtmlExtractor, HtmlQuery, Link, html_attr, html_attrs,
    html_fragment_attr, html_fragment_attrs, html_fragment_text, html_fragment_texts, html_text,
    html_texts,
};
pub use json::{required_json_array, required_json_item_string, required_json_string};
