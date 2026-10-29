//! 原生 JSON 与二进制文件的原子替换。

use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::Path;

/// 读取 JSON 文件；文件不存在时返回默认值。
///
/// # 参数
/// - `path`：JSON 文件路径。
/// - `label`：错误消息中的文件类型名称。
///
/// # 错误
/// 文件读取或 JSON 反序列化失败时返回错误消息。
pub(crate) fn read_or_default<T>(path: &Path, label: &str) -> Result<T, String>
where
    T: DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let contents = fs::read_to_string(path).map_err(|error| format!("无法读取{label}：{error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("{label}格式无效：{error}"))
}

/// 通过临时文件替换方式写入 JSON。
///
/// # 参数
/// - `path`：目标 JSON 文件路径。
/// - `value`：需要序列化的值。
/// - `label`：错误消息中的文件类型名称。
///
/// # 错误
/// 目录创建、序列化、写入或替换失败时返回错误消息。
pub(crate) fn write_atomically<T>(path: &Path, value: &T, label: &str) -> Result<(), String>
where
    T: Serialize,
{
    let parent = path
        .parent()
        .ok_or_else(|| format!("无法解析{label}目录"))?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建{label}目录：{error}"))?;
    let payload =
        serde_json::to_vec_pretty(value).map_err(|error| format!("无法序列化{label}：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, payload).map_err(|error| format!("无法写入{label}：{error}"))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("无法替换{label}：{error}"))?;
    }
    fs::rename(&temporary, path).map_err(|error| format!("无法完成{label}写入：{error}"))
}

/// 通过临时文件替换方式写入二进制文件。
///
/// # 参数
/// - `target`：目标文件路径。
/// - `payload`：需要写入的二进制内容。
/// - `label`：错误消息中的文件类型名称。
///
/// # 错误
/// 目录创建、写入或替换失败时返回错误消息。
pub(crate) fn write_bytes_atomically(
    target: &Path,
    payload: &[u8],
    label: &str,
) -> Result<(), String> {
    let parent = target.parent().ok_or_else(|| format!("{label}目录无效"))?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建{label}目录：{error}"))?;
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("part");
    let partial = target.with_extension(format!("{extension}.part"));
    fs::write(&partial, payload).map_err(|error| format!("无法写入{label}：{error}"))?;
    if target.exists() {
        fs::remove_file(target).map_err(|error| format!("无法替换{label}：{error}"))?;
    }
    fs::rename(&partial, target).map_err(|error| format!("无法完成{label}写入：{error}"))
}
