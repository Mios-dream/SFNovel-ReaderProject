//! 下载任务的内存注册表、取消标记与持久化快照。

use crate::adapters::storage::{read_job_snapshot, write_job_snapshot};
use crate::domain::download::{DownloadJob, DownloadSpec, JobSnapshot};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

#[derive(Default)]
/// 进程内下载任务、恢复参数、取消标记和作品写锁。
pub(crate) struct DownloadState {
    /// 以任务编号为键的公开任务状态。
    jobs: Mutex<HashMap<String, DownloadJob>>,
    /// 以任务编号为键的私有恢复参数。
    specs: Mutex<HashMap<String, DownloadSpec>>,
    /// 以任务编号为键的取消标记。
    cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// 防止同一小说的文字下载同时写入同一目录。
    text_locks: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
}

/// 任务快照中的公开状态与私有恢复参数。
type SnapshotParts = (HashMap<String, DownloadJob>, HashMap<String, DownloadSpec>);

/// 创建下载任务并登记取消标记与恢复参数。
///
/// # 参数
/// - `state`：共享下载状态。
/// - `kind`：媒体类型。
/// - `novel_id`：作品编号。
/// - `source_id`：可选的来源编号。
/// - `source_path`：可选的网页来源路径。
/// - `title`：任务展示名称。
/// - `chapter_ids`：待下载章节编号。
///
/// # 返回
/// 新任务及用于后台执行的取消标记。
///
/// # 错误
/// 系统时间无效或任一状态锁不可用时返回错误消息。
pub(super) fn create(
    state: &DownloadState,
    kind: &str,
    novel_id: i64,
    source_id: Option<i64>,
    source_path: Option<String>,
    title: String,
    chapter_ids: Vec<i64>,
) -> Result<(DownloadJob, Arc<AtomicBool>), String> {
    let id = format!(
        "{}-{novel_id}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "系统时间无效".to_string())?
            .as_millis()
    );
    let job = DownloadJob {
        id: id.clone(),
        title: title.clone(),
        kind: kind.to_string(),
        status: "queued".to_string(),
        progress: 0,
        message: "等待开始".to_string(),
        file: None,
        novel_id: Some(novel_id),
    };
    state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(id.clone(), job.clone());
    let cancelled = Arc::new(AtomicBool::new(false));
    state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(id.clone(), cancelled.clone());
    state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(
            id,
            DownloadSpec {
                kind: kind.to_string(),
                novel_id,
                source_id,
                source_path,
                title,
                chapter_ids,
            },
        );
    Ok((job, cancelled))
}

/// 在任务存在时原地更新其状态。
///
/// # 参数
/// - `state`：共享下载状态。
/// - `job_id`：任务唯一标识。
/// - `update`：只修改单个任务的闭包。
pub(super) fn update<F>(state: &DownloadState, job_id: &str, update: F)
where
    F: FnOnce(&mut DownloadJob),
{
    if let Ok(mut jobs) = state.jobs.lock() {
        if let Some(job) = jobs.get_mut(job_id) {
            update(job);
        }
    }
}

/// 读取单个任务的当前副本。
///
/// # 参数
/// - `state`：共享下载状态。
/// - `job_id`：任务唯一标识。
///
/// # 错误
/// 任务不存在或状态锁不可用时返回错误消息。
pub(super) fn get(state: &DownloadState, job_id: &str) -> Result<DownloadJob, String> {
    state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .get(job_id)
        .cloned()
        .ok_or_else(|| "下载任务不存在".to_string())
}

/// 列出全部任务，并按编号从新到旧排序。
///
/// # 错误
/// 状态锁不可用时返回错误消息。
pub(super) fn list(state: &DownloadState) -> Result<Vec<DownloadJob>, String> {
    let mut jobs = state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .values()
        .cloned()
        .collect::<Vec<_>>();
    jobs.sort_by(|left, right| right.id.cmp(&left.id));
    Ok(jobs)
}

/// 向渲染进程发送单个任务的进度事件。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `state`：共享下载状态。
/// - `job_id`：任务唯一标识。
pub(super) fn emit_job(app: &tauri::AppHandle, state: &DownloadState, job_id: &str) {
    if let Ok(job) = get(state, job_id) {
        let _ = app.emit("download-progress", job);
    }
}

/// 通知渲染进程刷新本地书库。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `state`：共享下载状态。
/// - `job_id`：产生书库变化的任务。
pub(super) fn emit_library(app: &tauri::AppHandle, state: &DownloadState, job_id: &str) {
    if let Ok(job) = get(state, job_id) {
        let _ = app.emit("download-library-update", job);
    }
}

/// 将当前任务和恢复参数写入磁盘快照。
///
/// # 错误
/// 状态锁不可用或快照写入失败时返回错误消息。
pub(super) fn persist(app: &tauri::AppHandle, state: &DownloadState) -> Result<(), String> {
    let (jobs, specs) = snapshot(state)?;
    write_job_snapshot(app, &JobSnapshot { jobs, specs })
}

/// 克隆当前任务与恢复参数，供持久化使用。
///
/// # 错误
/// 任一状态锁不可用时返回错误消息。
pub(super) fn snapshot(state: &DownloadState) -> Result<SnapshotParts, String> {
    let jobs = state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .clone();
    let specs = state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .clone();
    Ok((jobs, specs))
}

/// 从磁盘恢复任务，并把中断中的任务标记为可继续的暂停状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 快照读取、状态锁或回写失败时返回错误消息。
pub(super) fn restore(app: &tauri::AppHandle) -> Result<(), String> {
    let mut persisted = read_job_snapshot(app)?;
    persisted
        .jobs
        .retain(|id, _| persisted.specs.contains_key(id));
    for job in persisted.jobs.values_mut() {
        if matches!(job.status.as_str(), "queued" | "downloading") {
            job.status = "paused".to_string();
            job.message = "应用重启后已暂停，可继续下载".to_string();
        }
    }
    let state = app.state::<DownloadState>();
    *state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())? = persisted.jobs;
    *state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())? = persisted.specs;
    persist(app, &state)
}

/// 请求暂停任务并更新可展示状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `state`：共享下载状态。
/// - `job_id`：任务唯一标识。
///
/// # 错误
/// 任务不存在或状态持久化失败时返回错误消息。
pub(super) fn pause(
    app: &tauri::AppHandle,
    state: &DownloadState,
    job_id: &str,
) -> Result<DownloadJob, String> {
    let cancellation = state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .get(job_id)
        .cloned()
        .ok_or_else(|| "下载任务不存在".to_string())?;
    cancellation.store(true, Ordering::Relaxed);
    let result = {
        let mut jobs = state
            .jobs
            .lock()
            .map_err(|_| "下载任务状态不可用".to_string())?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| "下载任务不存在".to_string())?;
        if job.status == "queued" || job.status == "downloading" {
            job.status = "paused".to_string();
            job.message = "正在暂停下载".to_string();
        }
        job.clone()
    };
    persist(app, state)?;
    Ok(result)
}

/// 校验任务可恢复性并生成新的取消标记。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `state`：共享下载状态。
/// - `job_id`：任务唯一标识。
///
/// # 返回
/// 任务副本、持久化恢复参数和新的取消标记。
///
/// # 错误
/// 任务不是暂停状态、恢复参数丢失或状态持久化失败时返回错误消息。
pub(super) fn prepare_resume(
    app: &tauri::AppHandle,
    state: &DownloadState,
    job_id: &str,
) -> Result<(DownloadJob, DownloadSpec, Arc<AtomicBool>), String> {
    let job = get(state, job_id)?;
    if job.status != "paused" {
        return Err("只有已暂停的任务可以继续".to_string());
    }
    let spec = state
        .specs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .get(job_id)
        .cloned()
        .ok_or_else(|| "任务参数已过期，无法继续下载".to_string())?;
    let cancelled = Arc::new(AtomicBool::new(false));
    state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .insert(job_id.to_string(), cancelled.clone());
    update(state, job_id, |job| {
        job.status = "queued".to_string();
        job.message = "等待继续".to_string();
    });
    persist(app, state)?;
    Ok((job, spec, cancelled))
}

/// 删除任务、恢复参数和取消标记。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `state`：共享下载状态。
/// - `job_id`：任务唯一标识。
///
/// # 错误
/// 任务不存在或删除后的快照写入失败时返回错误消息。
pub(super) fn delete(
    app: &tauri::AppHandle,
    state: &DownloadState,
    job_id: &str,
) -> Result<(), String> {
    if let Some(cancellation) = state
        .cancellations
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .remove(job_id)
    {
        cancellation.store(true, Ordering::Relaxed);
    }
    let deleted = state
        .jobs
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .remove(job_id)
        .map(|_| ())
        .ok_or_else(|| "下载任务不存在".to_string());
    if deleted.is_ok() {
        let _ = state.specs.lock().map(|mut specs| specs.remove(job_id));
        persist(app, state)?;
    }
    deleted
}

/// 取得某本小说专用文字下载写锁。
///
/// # 参数
/// - `state`：共享下载状态。
/// - `novel_id`：小说编号。
///
/// # 错误
/// 状态锁不可用时返回错误消息。
pub(super) fn text_lock(
    state: &DownloadState,
    novel_id: i64,
) -> Result<Arc<tokio::sync::Mutex<()>>, String> {
    Ok(state
        .text_locks
        .lock()
        .map_err(|_| "下载任务状态不可用".to_string())?
        .entry(novel_id)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone())
}
