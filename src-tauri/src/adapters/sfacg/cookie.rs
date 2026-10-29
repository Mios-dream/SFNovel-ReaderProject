//! Cookie 请求头的最小化处理。

/// 从 Cookie 请求头中保留白名单内的键值对。
pub(crate) fn filter_cookie_header(cookie: &str, allowed_names: &[&str]) -> Option<String> {
    let pairs = cookie
        .split(';')
        .filter_map(|part| {
            let pair = part.trim();
            let (name, value) = pair.split_once('=')?;
            (!name.is_empty() && allowed_names.contains(&name)).then_some(format!("{name}={value}"))
        })
        .collect::<Vec<_>>();
    (!pairs.is_empty()).then(|| pairs.join("; "))
}

/// 判断经过白名单过滤的 Cookie 头是否包含指定会话键。
pub(crate) fn has_cookie_name(cookie: &str, name: &str) -> bool {
    cookie.split(';').any(|part| {
        part.trim()
            .split_once('=')
            .is_some_and(|(candidate, _)| candidate == name)
    })
}

#[cfg(test)]
mod tests {
    use super::{filter_cookie_header, has_cookie_name};

    #[test]
    fn only_allowed_cookie_names_are_forwarded() {
        let filtered = filter_cookie_header(
            "SFSession=abc; analytics=1; AuthKey=def; empty=",
            &["SFSession", "AuthKey"],
        );

        assert_eq!(filtered.as_deref(), Some("SFSession=abc; AuthKey=def"));
        assert_eq!(filter_cookie_header("analytics=1", &["SFSession"]), None);
    }

    #[test]
    fn cookie_name_matching_is_exact() {
        assert!(has_cookie_name("SFSession=abc; other=1", "SFSession"));
        assert!(!has_cookie_name("SFSession=abc; other=1", "Session"));
    }
}
