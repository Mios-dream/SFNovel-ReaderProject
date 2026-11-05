//! 原生 JSON 与二进制文件的原子替换。

use crate::domain::error::{AppError, AppResult};
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// 读取 JSON 文件；文件不存在时返回默认值。
pub(crate) fn read_or_default<T>(path: &Path, label: &str) -> AppResult<T>
where
    T: DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| AppError::storage(format!("无法读取{label}：{error}")))?;
    serde_json::from_str(&contents)
        .map_err(|error| AppError::storage(format!("{label}格式无效：{error}")))
}

/// 序列化并通过临时文件替换写入 JSON。
pub(crate) fn write_atomically<T>(path: &Path, value: &T, label: &str) -> AppResult<()>
where
    T: Serialize,
{
    let payload = serde_json::to_vec_pretty(value)
        .map_err(|error| AppError::storage(format!("无法序列化{label}：{error}")))?;
    replace_file(path, &payload, label)
}

/// 通过临时文件替换写入二进制内容。
///
/// 先写入并刷盘唯一临时文件，再整体替换目标文件：任何时刻目标文件要么是旧内容，
/// 要么是新内容，不会出现缺失窗口。
pub(crate) fn replace_file(target: &Path, payload: &[u8], label: &str) -> AppResult<()> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::storage(format!("{label}目录无效")))?;
    fs::create_dir_all(parent)
        .map_err(|error| AppError::storage(format!("无法创建{label}目录：{error}")))?;
    let temporary = temporary_path(target);
    write_and_sync(&temporary, payload, label)?;
    commit(&temporary, target, label)
}

/// 与目标同目录的唯一临时文件，避免并发写入互相覆盖。
fn temporary_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    target.with_file_name(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()))
}

/// 写入内容并确保落盘，避免重命名后内容仍在页缓存。
fn write_and_sync(path: &Path, payload: &[u8], label: &str) -> AppResult<()> {
    let mut file = fs::File::create(path)
        .map_err(|error| AppError::storage(format!("无法写入{label}：{error}")))?;
    file.write_all(payload)
        .map_err(|error| AppError::storage(format!("无法写入{label}：{error}")))?;
    file.sync_all()
        .map_err(|error| AppError::storage(format!("无法刷新{label}：{error}")))
}

#[cfg(windows)]
fn commit(temporary: &Path, target: &Path, label: &str) -> AppResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let from: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let replaced = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        let error = std::io::Error::last_os_error();
        let _ = fs::remove_file(temporary);
        return Err(AppError::storage(format!("无法完成{label}写入：{error}")));
    }
    Ok(())
}

#[cfg(not(windows))]
fn commit(temporary: &Path, target: &Path, label: &str) -> AppResult<()> {
    fs::rename(temporary, target).map_err(|error| {
        let _ = fs::remove_file(temporary);
        format!("无法完成{label}写入：{error}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::storage::test_support::TempDir;
    use crate::domain::error::ErrorKind;

    #[test]
    fn replace_file_overwrites_existing_content() {
        let dir = TempDir::new("replace");
        let target = dir.join("book.json");

        replace_file(&target, b"first", "测试文件").expect("首次写入");
        replace_file(&target, b"second", "测试文件").expect("覆盖写入");

        assert_eq!(fs::read(&target).expect("读取结果"), b"second");
    }

    #[test]
    fn replace_file_leaves_no_temporary_file() {
        let dir = TempDir::new("cleanup");
        let target = dir.join("payload.bin");

        replace_file(&target, b"payload", "测试文件").expect("写入");

        let leftovers = fs::read_dir(&dir.0)
            .expect("列出目录")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0, "替换完成后不应残留临时文件");
    }

    #[test]
    fn replace_file_creates_missing_parent_directories() {
        let dir = TempDir::new("parents");
        let target = dir.join("nested/deeper/book.json");

        replace_file(&target, b"{}", "测试文件").expect("写入嵌套路径");

        assert!(target.is_file());
    }

    #[test]
    fn read_or_default_returns_default_for_missing_file() {
        let dir = TempDir::new("missing");
        let target = dir.join("absent.json");

        let value = read_or_default::<serde_json::Value>(&target, "测试文件").expect("读取");

        assert!(value.is_null(), "缺失文件应返回默认值");
    }

    #[test]
    fn read_or_default_reports_malformed_json_as_storage_error() {
        let dir = TempDir::new("malformed");
        let target = dir.join("broken.json");
        fs::write(&target, b"{not json").expect("写入坏数据");

        let error = read_or_default::<serde_json::Value>(&target, "测试文件").expect_err("应失败");

        assert_eq!(error.kind(), ErrorKind::Storage);
        assert!(
            error.to_string().contains("格式无效"),
            "错误消息应说明格式问题"
        );
    }

    #[test]
    fn write_atomically_round_trips_through_read_or_default() {
        let dir = TempDir::new("roundtrip");
        let target = dir.join("settings.json");
        let value = serde_json::json!({ "download": { "concurrency": 3 } });

        write_atomically(&target, &value, "测试设置").expect("写入");
        let restored = read_or_default::<serde_json::Value>(&target, "测试设置").expect("读取");

        assert_eq!(restored, value);
    }

    #[test]
    fn write_failure_is_classified_as_storage_not_internal() {
        let dir = TempDir::new("write-failure");
        let blocker = dir.join("blocker");
        fs::write(&blocker, b"file").expect("创建占位文件");
        // 父路径已是普通文件，创建目录必然失败。
        let target = dir.join("blocker").join("settings.json");

        let error =
            write_atomically(&target, &serde_json::json!({}), "测试设置").expect_err("应失败");

        assert_eq!(
            error.kind(),
            ErrorKind::Storage,
            "文件系统失败应归为存储错误，而不是内部错误"
        );
    }
}
