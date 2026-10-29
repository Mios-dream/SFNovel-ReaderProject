//! 请求策略、正文恢复字典与平台存储权限。

use super::atomic::{read_or_default, write_atomically};
use super::paths::{content_dictionary_path, request_policy_path};
use crate::domain::settings::{ContentDictionaryResult, RequestPolicy};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// 应用首次运行时使用的内置正文恢复字典。
const DEFAULT_CONTENT_DICTIONARY: &str = include_str!("../../../../sfacg-content-dictionary.json");

/// 读取请求策略；文件缺失时使用默认值。
///
/// # 错误
/// 策略文件读取或解析失败时返回错误消息。
pub(crate) fn load_policy(app: &tauri::AppHandle) -> Result<RequestPolicy, String> {
    read_or_default(&request_policy_path(app)?, "请求策略")
}

/// 原子保存请求策略。
///
/// # 错误
/// 路径解析、序列化或写入失败时返回错误消息。
pub(crate) fn save_policy(app: &tauri::AppHandle, policy: &RequestPolicy) -> Result<(), String> {
    write_atomically(&request_policy_path(app)?, policy, "请求策略")
}

/// 判断字符是否属于恢复字典允许的汉字区间。
///
/// # 参数
/// - `character`：待判断的字符。
fn is_han_character(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}'
    )
}

/// 过滤恢复字典，只保留单字到单字的汉字映射。
///
/// # 参数
/// - `value`：从 JSON 文件读取的任意值。
fn valid_content_dictionary(value: Value) -> HashMap<String, String> {
    let Some(entries) = value.as_object() else {
        return HashMap::new();
    };
    entries
        .iter()
        .filter_map(|(key, value)| {
            let replacement = value.as_str()?;
            if key.chars().count() != 1
                || replacement.chars().count() != 1
                || !key.chars().all(is_han_character)
                || !replacement.chars().all(is_han_character)
            {
                return None;
            }
            Some((key.clone(), replacement.to_string()))
        })
        .collect()
}

/// 读取用户字典；文件缺失时写入并返回内置字典。
///
/// # 错误
/// 字典读取、JSON 解析或默认字典初始化写入失败时返回错误消息。
pub(crate) fn load_content_dictionary(
    app: &tauri::AppHandle,
) -> Result<HashMap<String, String>, String> {
    let path = content_dictionary_path(app)?;
    if path.exists() {
        let contents =
            fs::read_to_string(&path).map_err(|error| format!("无法读取正文恢复字典：{error}"))?;
        let value = serde_json::from_str::<Value>(&contents)
            .map_err(|error| format!("正文恢复字典格式无效：{error}"))?;
        return Ok(valid_content_dictionary(value));
    }
    let defaults = valid_content_dictionary(
        serde_json::from_str(DEFAULT_CONTENT_DICTIONARY)
            .map_err(|error| format!("内置正文恢复字典格式无效：{error}"))?,
    );
    write_content_dictionary(app, &defaults)?;
    Ok(defaults)
}

/// 原子写入正文恢复字典。
///
/// # 错误
/// 路径解析或文件写入失败时返回错误消息。
pub(crate) fn write_content_dictionary(
    app: &tauri::AppHandle,
    dictionary: &HashMap<String, String>,
) -> Result<(), String> {
    write_atomically(&content_dictionary_path(app)?, dictionary, "正文恢复字典")
}

/// 统计正文恢复字典条目数。
///
/// # 错误
/// 字典无法读取时返回错误消息。
pub(crate) fn dictionary_size(app: &tauri::AppHandle) -> Result<ContentDictionaryResult, String> {
    let dictionary = load_content_dictionary(app)?;
    Ok(ContentDictionaryResult {
        size: dictionary.len(),
        added: None,
    })
}

/// 使用恢复字典替换 App 正文中的单字混淆。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `content`：待解码的正文。
///
/// # 错误
/// 恢复字典无法读取时返回错误消息。
pub(crate) fn decode_api_content(app: &tauri::AppHandle, content: &str) -> Result<String, String> {
    let dictionary = load_content_dictionary(app)?;
    Ok(content
        .chars()
        .map(|character| {
            dictionary
                .get(&character.to_string())
                .and_then(|value| value.chars().next())
                .unwrap_or(character)
        })
        .collect())
}

/// 在策略启用外部存储时请求平台权限。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 策略读取失败或平台权限请求失败时返回错误消息。
pub(crate) async fn ensure_external_storage_access(app: &tauri::AppHandle) -> Result<bool, String> {
    if !load_policy(app)?.use_external_storage {
        return Ok(true);
    }
    #[cfg(target_os = "android")]
    {
        return crate::adapters::platform::android::ensure_external_storage_access(app).await;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(true)
    }
}

/// 创建唯一导出临时文件路径。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `extension`：临时文件扩展名。
///
/// # 错误
/// 系统临时目录无法解析时返回错误消息。
pub(crate) fn export_temp_path(app: &tauri::AppHandle, extension: &str) -> Result<PathBuf, String> {
    use tauri::Manager;
    app.path()
        .temp_dir()
        .map(|directory| {
            directory.join(format!(".sf-export-{}.{}", uuid::Uuid::new_v4(), extension))
        })
        .map_err(|error| format!("无法解析导出临时目录：{error}"))
}

/// 把临时导出文件写入 Android `content://` 目标。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `source`：已完成写入的本地临时文件。
/// - `uri`：Android 文档提供器返回的目标 URI。
///
/// # 错误
/// 当前平台不是 Android 或平台写入失败时返回错误消息。
pub(crate) async fn android_write_export(
    _app: &tauri::AppHandle,
    _source: &std::path::Path,
    _uri: &str,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return crate::adapters::platform::android::write_export_to_uri(_app, _source, _uri).await;
    #[cfg(not(target_os = "android"))]
    Err("当前平台不支持 content:// 导出位置".to_string())
}

#[cfg(test)]
mod tests {
    use super::valid_content_dictionary;
    use serde_json::json;

    #[test]
    fn content_dictionary_requires_single_han_characters() {
        let dictionary = valid_content_dictionary(json!({
            "哪": "那",
            "字": "词",
            "ab": "缺",
            "A": "中",
            "好": "好呀",
            "𠀀": "一"
        }));

        assert_eq!(dictionary.len(), 2);
        assert_eq!(dictionary.get("哪").map(String::as_str), Some("那"));
        assert_eq!(dictionary.get("字").map(String::as_str), Some("词"));
    }
}
