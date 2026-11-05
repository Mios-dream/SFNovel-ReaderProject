//! 下载任务的内存注册表、取消标记与持久化快照。

use crate::adapters::storage::{read_job_snapshot, write_job_snapshot};
use crate::domain::download::{DownloadJob, DownloadSpec, JobSnapshot, JobStatus, MediaKind};
use crate::domain::error::{AppError, AppResult};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
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

impl DownloadState {
    fn jobs(&self) -> AppResult<MutexGuard<'_, HashMap<String, DownloadJob>>> {
        self.jobs
            .lock()
            .map_err(|_| AppError::internal("下载任务状态不可用"))
    }

    fn specs(&self) -> AppResult<MutexGuard<'_, HashMap<String, DownloadSpec>>> {
        self.specs
            .lock()
            .map_err(|_| AppError::internal("下载任务状态不可用"))
    }

    fn cancellations(&self) -> AppResult<MutexGuard<'_, HashMap<String, Arc<AtomicBool>>>> {
        self.cancellations
            .lock()
            .map_err(|_| AppError::internal("下载任务状态不可用"))
    }

    fn text_locks(&self) -> AppResult<MutexGuard<'_, HashMap<i64, Arc<tokio::sync::Mutex<()>>>>> {
        self.text_locks
            .lock()
            .map_err(|_| AppError::internal("下载任务状态不可用"))
    }
}

/// 任务快照中的公开状态与私有恢复参数。
type SnapshotParts = (HashMap<String, DownloadJob>, HashMap<String, DownloadSpec>);

/// 创建下载任务并登记取消标记与恢复参数。
pub(super) fn create(
    state: &DownloadState,
    kind: MediaKind,
    novel_id: i64,
    source_id: Option<i64>,
    source_path: Option<String>,
    title: String,
    chapter_ids: Vec<i64>,
) -> AppResult<(DownloadJob, Arc<AtomicBool>)> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::internal("系统时间无效"))?
        .as_millis();
    let id = format!("{milliseconds}-{novel_id}");
    let job = DownloadJob {
        id: id.clone(),
        title: title.clone(),
        kind,
        status: JobStatus::Queued,
        progress: 0,
        message: "等待开始".to_string(),
        file: None,
        novel_id: Some(novel_id),
    };
    state.jobs()?.insert(id.clone(), job.clone());
    let cancelled = Arc::new(AtomicBool::new(false));
    state.cancellations()?.insert(id.clone(), cancelled.clone());
    state.specs()?.insert(
        id,
        DownloadSpec {
            kind,
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
pub(super) fn update<F>(state: &DownloadState, job_id: &str, update: F)
where
    F: FnOnce(&mut DownloadJob),
{
    if let Ok(mut jobs) = state.jobs() {
        if let Some(job) = jobs.get_mut(job_id) {
            update(job);
        }
    }
}

/// 读取单个任务的当前副本。
pub(super) fn get(state: &DownloadState, job_id: &str) -> AppResult<DownloadJob> {
    state
        .jobs()?
        .get(job_id)
        .cloned()
        .ok_or_else(|| AppError::not_found("下载任务不存在"))
}

/// 列出全部任务，并按编号从新到旧排序。
pub(super) fn list(state: &DownloadState) -> AppResult<Vec<DownloadJob>> {
    let mut jobs = state.jobs()?.values().cloned().collect::<Vec<_>>();
    jobs.sort_by(|left, right| right.id.cmp(&left.id));
    Ok(jobs)
}

/// 向渲染进程发送单个任务的进度事件。
pub(super) fn emit_job(app: &tauri::AppHandle, state: &DownloadState, job_id: &str) {
    if let Ok(job) = get(state, job_id) {
        let _ = app.emit("download-progress", job);
    }
}

/// 通知渲染进程刷新本地书库。
pub(super) fn emit_library(app: &tauri::AppHandle, state: &DownloadState, job_id: &str) {
    if let Ok(job) = get(state, job_id) {
        let _ = app.emit("download-library-update", job);
    }
}

/// 将当前任务和恢复参数写入磁盘快照。
pub(super) fn persist(app: &tauri::AppHandle, state: &DownloadState) -> AppResult<()> {
    let (jobs, specs) = snapshot(state)?;
    write_job_snapshot(app, &JobSnapshot { jobs, specs })
}

/// 克隆当前任务与恢复参数，供持久化使用。
pub(super) fn snapshot(state: &DownloadState) -> AppResult<SnapshotParts> {
    Ok((state.jobs()?.clone(), state.specs()?.clone()))
}

/// 从磁盘恢复任务，并把中断中的任务标记为可继续的暂停状态。
pub(super) fn restore(app: &tauri::AppHandle) -> AppResult<()> {
    let mut persisted = read_job_snapshot(app)?;
    persisted
        .jobs
        .retain(|id, _| persisted.specs.contains_key(id));
    for job in persisted.jobs.values_mut() {
        if job.status.is_active() {
            job.status = JobStatus::Paused;
            job.message = "应用重启后已暂停，可继续下载".to_string();
        }
    }
    let state = app.state::<DownloadState>();
    *state.jobs()? = persisted.jobs;
    *state.specs()? = persisted.specs;
    persist(app, &state)
}

/// 请求暂停任务并更新可展示状态。
pub(super) fn pause(
    app: &tauri::AppHandle,
    state: &DownloadState,
    job_id: &str,
) -> AppResult<DownloadJob> {
    let cancellation = state
        .cancellations()?
        .get(job_id)
        .cloned()
        .ok_or_else(|| AppError::not_found("下载任务不存在"))?;
    cancellation.store(true, Ordering::Relaxed);
    let result = {
        let mut jobs = state.jobs()?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| AppError::not_found("下载任务不存在"))?;
        if job.status.is_active() {
            job.status = JobStatus::Paused;
            job.message = "正在暂停下载".to_string();
        }
        job.clone()
    };
    persist(app, state)?;
    Ok(result)
}

/// 校验任务可恢复性并生成新的取消标记。
pub(super) fn prepare_resume(
    app: &tauri::AppHandle,
    state: &DownloadState,
    job_id: &str,
) -> AppResult<(DownloadJob, DownloadSpec, Arc<AtomicBool>)> {
    let job = get(state, job_id)?;
    if job.status != JobStatus::Paused {
        return Err(AppError::invalid_input("只有已暂停的任务可以继续"));
    }
    let spec = state
        .specs()?
        .get(job_id)
        .cloned()
        .ok_or_else(|| AppError::not_found("任务参数已过期，无法继续下载"))?;
    let cancelled = Arc::new(AtomicBool::new(false));
    state
        .cancellations()?
        .insert(job_id.to_string(), cancelled.clone());
    update(state, job_id, |job| {
        job.status = JobStatus::Queued;
        job.message = "等待继续".to_string();
    });
    persist(app, state)?;
    Ok((job, spec, cancelled))
}

/// 删除任务、恢复参数和取消标记。
pub(super) fn delete(app: &tauri::AppHandle, state: &DownloadState, job_id: &str) -> AppResult<()> {
    if let Some(cancellation) = state.cancellations()?.remove(job_id) {
        cancellation.store(true, Ordering::Relaxed);
    }
    let deleted = state
        .jobs()?
        .remove(job_id)
        .map(|_| ())
        .ok_or_else(|| AppError::not_found("下载任务不存在"));
    if deleted.is_ok() {
        state.specs()?.remove(job_id);
        persist(app, state)?;
    }
    deleted
}

/// 取得某本小说专用文字下载写锁。
pub(super) fn text_lock(
    state: &DownloadState,
    novel_id: i64,
) -> AppResult<Arc<tokio::sync::Mutex<()>>> {
    Ok(state
        .text_locks()?
        .entry(novel_id)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone())
}
