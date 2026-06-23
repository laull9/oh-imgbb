//! analysis 模块提供页面内容提取和后续分析能力。

pub mod extract;
pub mod json;

pub use extract::{ExtractResult, HtmlExtractor, Link};
pub use json::{required_json_array, required_json_item_string, required_json_string};
