//! SF 网页片段的轻量 HTML 与 JavaScript 字段解析。

/// 从 HTML 片段读取首个指定属性的非负整数。
///
/// # 参数
/// - `value`：待搜索的 HTML 片段。
/// - `attribute`：属性名，不带等号。
///
/// # 返回
/// 返回带引号或不带引号属性值中的前导数字。
pub(super) fn extract_numeric_attribute(value: &str, attribute: &str) -> Option<i64> {
    let marker = format!("{attribute}=");
    let start = value.find(&marker)? + marker.len();
    let tail = value[start..].trim_start();
    let tail = tail.strip_prefix('"').or_else(|| tail.strip_prefix('\''));
    let digits = match tail {
        Some(value) => value
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>(),
        None => value
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>(),
    };
    digits.parse::<i64>().ok()
}

/// 移除简单 HTML 标签和常见实体，将连续空白折叠为单个空格。
pub(super) fn strip_html_text(value: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 从单个已限定范围的 HTML 标签读取带引号或不带引号的属性。
pub(super) fn html_tag_attribute(tag: &str, name: &str) -> Option<String> {
    let marker = format!("{name}=");
    let position = tag.to_ascii_lowercase().find(&marker)? + marker.len();
    let value = tag[position..].trim_start();
    if let Some(quote) = value
        .chars()
        .next()
        .filter(|value| *value == '\'' || *value == '"')
    {
        let end = value[1..].find(quote)? + 1;
        return Some(value[1..end].to_string());
    }
    let end = value
        .find(char::is_whitespace)
        .or_else(|| value.find('>'))
        .unwrap_or(value.len());
    (!value[..end].is_empty()).then(|| value[..end].to_string())
}

/// 从官方登录信息端点返回的短 JavaScript 响应中提取字段。
pub(super) fn extract_js_field(value: &str, field: &str) -> Option<String> {
    let marker = format!("{field}:");
    let marker_start = value.find(&marker)?;
    let rest = value[marker_start + marker.len()..].trim_start();
    if let Some(quote) = rest.chars().next().filter(|c| *c == '\'' || *c == '"') {
        let start = quote.len_utf8();
        let end = rest[start..].find(quote)? + start;
        return (!rest[start..end].trim().is_empty()).then(|| rest[start..end].trim().to_string());
    }
    let token = rest
        .split(|character: char| {
            character == ',' || character == '}' || character == ']' || character.is_whitespace()
        })
        .next()
        .unwrap_or("")
        .trim();
    (!token.is_empty()).then(|| token.to_string())
}
