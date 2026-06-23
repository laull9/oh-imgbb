//! json 模块提供 JSON 结构读取辅助能力。

use anyhow::{Result, anyhow};
use serde_json::Value;

/// 读取 JSON 指针位置上的非空字符串。
pub fn required_json_string<'a>(value: &'a Value, pointer: &str) -> Result<&'a str> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("JSON 缺少非空字符串字段: {pointer}"))
}

/// 读取 JSON 指针位置上的数组。
pub fn required_json_array<'a>(value: &'a Value, pointer: &str) -> Result<&'a Vec<Value>> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("JSON 缺少数组字段: {pointer}"))
}

/// 读取数组条目中的非空字符串字段。
pub fn required_json_item_string<'a>(
    item: &'a Value,
    array_name: &str,
    index: usize,
    key: &str,
) -> Result<&'a str> {
    item.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("{array_name}[{index}].{key} 缺少非空字符串"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证 JSON 指针可以读取非空字符串。
    #[test]
    fn required_json_string_reads_trimmed_value() {
        let value = json!({ "album": { "name": " demo " } });

        assert_eq!(required_json_string(&value, "/album/name").unwrap(), "demo");
    }

    /// 验证 JSON 指针会拒绝空字符串。
    #[test]
    fn required_json_string_rejects_empty_value() {
        let value = json!({ "album": { "name": " " } });

        assert!(required_json_string(&value, "/album/name").is_err());
    }

    /// 验证 JSON 指针可以读取数组。
    #[test]
    fn required_json_array_reads_array() {
        let value = json!({ "contents": [{ "filename": "one.jpg" }] });

        assert_eq!(required_json_array(&value, "/contents").unwrap().len(), 1);
    }

    /// 验证数组条目可以读取非空字符串。
    #[test]
    fn required_json_item_string_reads_field() {
        let item = json!({ "url": " https://example.com/a.jpg " });

        assert_eq!(
            required_json_item_string(&item, "contents", 0, "url").unwrap(),
            "https://example.com/a.jpg"
        );
    }
}
