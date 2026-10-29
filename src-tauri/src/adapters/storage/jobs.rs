//! 下载任务快照文件。

use super::atomic::{read_or_default, write_atomically};
use super::paths::job_snapshot_path;
use crate::domain::download::JobSnapshot;

/// 读取下载任务快照；文件不存在时返回空快照。
///
/// # 错误
/// 路径解析、文件读取或 JSON 解析失败时返回错误消息。
pub(crate) fn read_job_snapshot(app: &tauri::AppHandle) -> Result<JobSnapshot, String> {
    let path = job_snapshot_path(app)?;
    if !path.exists() {
        return Ok(JobSnapshot::default());
    }
    read_or_default(&path, "下载任务")
}

/// 原子写入下载任务快照。
///
/// # 错误
/// 路径解析、序列化或写入失败时返回错误消息。
pub(crate) fn write_job_snapshot(
    app: &tauri::AppHandle,
    snapshot: &JobSnapshot,
) -> Result<(), String> {
    write_atomically(&job_snapshot_path(app)?, snapshot, "下载任务")
}
