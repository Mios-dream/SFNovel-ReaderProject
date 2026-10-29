//! 应用自有目录的解析与名称约束。

use super::atomic::read_or_default;
use crate::domain::settings::RequestPolicy;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

/// 返回平台应用数据目录。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 平台无法解析应用数据目录时返回错误消息。
pub(crate) fn app_data_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("无法解析应用数据目录：{error}"))
}

/// 按请求策略返回书库目录并确保其存在。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 策略读取失败或目录无法创建时返回错误消息。
pub(crate) fn library_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let policy = read_or_default::<RequestPolicy>(&request_policy_path(app)?, "请求策略")?;
    let directory = if cfg!(target_os = "android") && policy.use_external_storage {
        PathBuf::from("/storage/emulated/0/Download/SF Novel Flow")
    } else {
        app_data_directory(app)?.join("library")
    };
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建本地书库目录：{error}"))?;
    Ok(directory)
}

/// 校验书名并解析书库内的真实书籍目录。
///
/// 该函数会拒绝路径分隔符、上级跳转和书库根目录之外的路径。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `name`：本地书籍目录名。
///
/// # 错误
/// 名称为空、包含路径片段、书籍不存在或解析后越出书库根目录时返回错误消息。
pub(crate) fn local_book_directory(app: &tauri::AppHandle, name: &str) -> Result<PathBuf, String> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains(['/', '\\', ':'])
        || trimmed.contains("..")
    {
        return Err("本地书籍目录无效".to_string());
    }
    let root = library_directory(app)?
        .canonicalize()
        .map_err(|error| format!("无法解析书库目录：{error}"))?;
    let canonical = root
        .join(trimmed)
        .canonicalize()
        .map_err(|_| "本地书籍不存在".to_string())?;
    if !canonical.starts_with(&root) || !canonical.is_dir() {
        return Err("本地书籍目录无效".to_string());
    }
    Ok(canonical)
}

/// 返回请求策略文件路径。
///
/// # 错误
/// 应用数据目录无法解析时返回错误消息。
pub(crate) fn request_policy_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_directory(app)?.join("request-policy.json"))
}

/// 返回正文恢复字典文件路径。
///
/// # 错误
/// 应用数据目录无法解析时返回错误消息。
pub(crate) fn content_dictionary_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_directory(app)?.join("sfacg-content-dictionary.json"))
}

/// 返回下载任务快照文件路径。
///
/// # 错误
/// 应用数据目录无法解析时返回错误消息。
pub(crate) fn job_snapshot_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_directory(app)?.join("download-jobs.json"))
}

/// 返回在线浏览缓存目录。
///
/// # 错误
/// 应用数据目录无法解析时返回错误消息。
pub(crate) fn online_cache_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_directory(app)?.join("online-cache"))
}

/// 将任意书名转换为非空、跨平台安全的目录名。
///
/// # 参数
/// - `value`：上游书名或用户输入的标题。
pub(crate) fn safe_library_name(value: &str) -> String {
    let mut result = value
        .chars()
        .map(|character| {
            if ['<', '>', ':', '"', '/', '\\', '|', '?', '*'].contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .trim_end_matches(['.', ' '])
        .to_string();
    if result.is_empty() {
        result = "未命名作品".to_string();
    }
    result.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::safe_library_name;

    #[test]
    fn library_names_are_filesystem_safe_and_non_empty() {
        assert_eq!(safe_library_name("a/b:c*?"), "a_b_c__");
        assert_eq!(safe_library_name("   "), "未命名作品");
        assert_eq!(safe_library_name("名字...  "), "名字");
    }
}
