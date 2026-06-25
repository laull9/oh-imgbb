use serde_json::Value;

/// 从接口响应中递归提取常见 ID。
pub(super) fn find_id(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            for key in ["id_encoded", "album_id", "image_id", "id"] {
                if let Some(id) = object.get(key).and_then(Value::as_str) {
                    return Some(id.to_string());
                }
            }
            object.values().find_map(find_id)
        }
        Value::Array(items) => items.iter().find_map(find_id),
        _ => None,
    }
}

/// 从接口响应中递归提取常见 URL。
pub(super) fn find_url(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            for key in ["url", "url_viewer", "display_url", "image_url"] {
                if let Some(url) = object.get(key).and_then(Value::as_str) {
                    return Some(url.to_string());
                }
            }
            object.values().find_map(find_url)
        }
        Value::Array(items) => items.iter().find_map(find_url),
        _ => None,
    }
}

/// 从接口响应中递归提取图片 ID。
pub(super) fn find_image_id(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            for key in ["image_id", "id_encoded", "id"] {
                if let Some(id) = object.get(key).and_then(Value::as_str) {
                    return Some(id.to_string());
                }
            }
            object.values().find_map(find_image_id)
        }
        Value::Array(items) => items.iter().find_map(find_image_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证响应摘要可以递归提取 ID 和 URL。
    #[test]
    fn report_helpers_extract_nested_values() {
        let value = json!({
            "status_code": 200,
            "album": { "id_encoded": "ABC123", "url": "https://ibb.co/album/ABC123" }
        });

        assert_eq!(find_id(&value), Some("ABC123".to_string()));
        assert_eq!(
            find_url(&value),
            Some("https://ibb.co/album/ABC123".to_string())
        );
    }

    /// 验证上传响应会优先提取图片 ID 而不是相册 ID。
    #[test]
    fn image_id_helper_ignores_album_id_priority() {
        let value = json!({
            "status_code": 200,
            "album_id": "ALBUM123",
            "image": { "image_id": "IMAGE123" }
        });

        assert_eq!(find_image_id(&value), Some("IMAGE123".to_string()));
    }
}
