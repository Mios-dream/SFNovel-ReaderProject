//! App API 成功响应的进程内缓存。

use serde_json::Value;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(300);
const CACHE_LIMIT: usize = 256;

static ENTRIES: LazyLock<Mutex<HashMap<String, Entry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct Entry {
    value: Value,
    expires_at: Instant,
}

/// 生成区分凭证、路径和查询参数的缓存键。
pub(super) fn key(
    include_session: bool,
    cookie: Option<&str>,
    path: &str,
    query: &[(&str, String)],
) -> String {
    let mut hasher = DefaultHasher::new();
    cookie.unwrap_or_default().hash(&mut hasher);
    let mut key = format!("{include_session}|{:x}|{path}", hasher.finish());
    for (name, value) in query {
        key.push('&');
        key.push_str(name);
        key.push('=');
        key.push_str(value);
    }
    key
}

/// 读取未过期的缓存响应。
pub(super) fn get(key: &str) -> Option<Value> {
    let mut entries = ENTRIES.lock().ok()?;
    let entry = entries.get(key)?;
    if entry.expires_at <= Instant::now() {
        entries.remove(key);
        return None;
    }
    Some(entry.value.clone())
}

/// 写入成功响应，并在条目过多时清理过期内容。
pub(super) fn insert(key: String, value: Value) {
    let now = Instant::now();
    let Ok(mut entries) = ENTRIES.lock() else {
        return;
    };
    if entries.len() >= CACHE_LIMIT {
        entries.retain(|_, entry| entry.expires_at > now);
        if entries.len() >= CACHE_LIMIT {
            entries.clear();
        }
    }
    entries.insert(
        key,
        Entry {
            value,
            expires_at: now + CACHE_TTL,
        },
    );
}

/// 删除一个缓存项。
pub(super) fn remove(key: &str) {
    if let Ok(mut entries) = ENTRIES.lock() {
        entries.remove(key);
    }
}
