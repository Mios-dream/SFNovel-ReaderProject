//! 本地 JSON 文件的安全读写工具。
//!
//! 路径必须由原生层解析，不能接受渲染进程提供的任意路径。写入先完成同目录临时
//! 文件，再替换目标文件，避免进程中断时将半写入 JSON 留在目标路径。

use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::Path;

/// 读取 JSON；文件不存在时返回类型默认值。
///
/// # 错误
/// 已存在文件无法读取或不能反序列化为 `T` 时返回带调用方前缀的错误。
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

/// 将值格式化为 JSON 后经同目录临时文件替换目标文件。
///
/// # 错误
/// 创建目录、序列化、写入或替换任何一步失败时返回带调用方前缀的错误。
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
