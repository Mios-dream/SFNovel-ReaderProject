//! Cookie 请求头的最小化处理工具。
//!
//! 这些函数只处理 `name=value` 形式的请求头片段，不解析或记录 Cookie 值。调用方
//! 必须提供端点所属会话可接受的名称，避免 App 与 Web 会话混用。

/// 从 Cookie 请求头中保留白名单内的键值对。
///
/// `allowed_names` 按精确 Cookie 名称匹配；格式错误、空名称和未列入白名单的片段
/// 都会被丢弃。返回值可直接用于 `Cookie` 请求头，绝不包含属性字段。
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
///
/// 此函数只比较键名，因此不会暴露或记录会话值。
pub(crate) fn has_cookie_name(cookie: &str, name: &str) -> bool {
    cookie.split(';').any(|part| {
        part.trim()
            .split_once('=')
            .is_some_and(|(candidate, _)| candidate == name)
    })
}
