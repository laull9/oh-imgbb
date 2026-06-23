use anyhow::{Result, anyhow, ensure};

/// 清理目录名和文件名中的路径分隔符。
pub(super) fn sanitize_path_segment(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(sanitize_path_char)
        .collect::<String>()
        .trim_matches(|ch| ch == '.' || ch == ' ')
        .to_string()
}

/// 将不适合作为文件名的字符替换为下划线。
fn sanitize_path_char(ch: char) -> char {
    match ch {
        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
        _ if ch.is_control() => '_',
        _ => ch,
    }
}

/// 规整用户输入 URL 的协议前缀。
pub(super) fn normalize_url_input(input: &str) -> String {
    let input = input.trim();
    if input.starts_with("http://") || input.starts_with("https://") {
        return input.to_string();
    }

    format!("https://{input}")
}

/// 从 embeds 页面提取 ImgBB 动态 auth_token。
pub(super) fn extract_auth_token(html: &str) -> Result<String> {
    let marker = "PF.obj.config.auth_token=\"";
    let start = html
        .find(marker)
        .map(|index| index + marker.len())
        .ok_or_else(|| anyhow!("未找到 ImgBB auth_token"))?;
    let rest = &html[start..];
    let end = rest
        .find('"')
        .ok_or_else(|| anyhow!("ImgBB auth_token 格式异常"))?;
    let auth_token = &rest[..end];
    ensure!(!auth_token.is_empty(), "ImgBB auth_token 为空");

    Ok(auth_token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证页面脚本中可以提取 auth_token。
    #[test]
    fn auth_token_can_be_extracted() {
        let html = r#"<script>PF.obj.config.auth_token="token-123";</script>"#;

        assert_eq!(extract_auth_token(html).unwrap(), "token-123");
    }
}
