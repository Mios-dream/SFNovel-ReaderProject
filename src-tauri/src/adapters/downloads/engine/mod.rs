//! 下载任务的后台执行器。
//!
//! 本模块只负责把持久化任务分派给对应媒体流程；任务状态和进度发布统一由
//! 上级注册表处理。

mod assets;
mod audio;
mod comic;
mod prepare;
mod text;

use crate::adapters::downloads::registry;
use crate::adapters::downloads::DownloadState;
use crate::domain::download::{DownloadSpec, JobStatus, MediaKind};
use crate::domain::error::AppResult;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// 并发上限的硬上限，同时也是信号量的初始容量。
const MAX_SLOTS: usize = 4;

/// 全进程共享的下载并发槽位；只创建一次，配置变更时增减 permit。
static DOWNLOAD_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// 当前允许的并发数，用于配置变更时计算增量。
static SLOT_LIMIT: AtomicUsize = AtomicUsize::new(MAX_SLOTS);

fn slots() -> &'static Arc<Semaphore> {
    DOWNLOAD_SLOTS.get_or_init(|| Arc::new(Semaphore::new(MAX_SLOTS)))
}

/// 更新下载任务并发上限。
///
/// 运行中的任务持有旧 permit，会晚于新上限释放；因此调低上限后短暂超出属预期。
pub(crate) fn configure_concurrency(max_concurrent: u8) {
    let desired = usize::from(max_concurrent.clamp(1, MAX_SLOTS as u8));
    let previous = SLOT_LIMIT.swap(desired, Ordering::SeqCst);
    if desired > previous {
        slots().add_permits(desired - previous);
    } else if desired < previous {
        let _ = slots().forget_permits(previous - desired);
    }
}

/// 等待并取得一个下载并发槽位。
async fn acquire_slot() -> Option<OwnedSemaphorePermit> {
    slots().clone().acquire_owned().await.ok()
}

/// 在取得并发槽位后执行单个下载任务。
fn spawn_with_slot<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let Some(_slot) = acquire_slot().await else {
            return;
        };
        future.await;
    });
}

/// 写入任务最终状态并持久化，三类下载共用同一套状态机。
pub(super) fn finish(
    context: &JobContext<'_>,
    result: AppResult<String>,
    done_message: &str,
    expose_file: bool,
) {
    let JobContext {
        app,
        state,
        job_id,
        cancelled,
    } = *context;
    match result {
        Ok(file) => registry::update(state, job_id, |job| {
            job.status = JobStatus::Done;
            job.progress = 100;
            job.message = done_message.to_string();
            if expose_file {
                job.file = Some(file);
            }
        }),
        Err(error) if error.is_cancelled() || cancelled.load(Ordering::Relaxed) => {
            registry::update(state, job_id, |job| {
                job.status = JobStatus::Paused;
                job.message = "已暂停，可继续下载".to_string();
            })
        }
        Err(error) => registry::update(state, job_id, |job| {
            job.status = JobStatus::Error;
            job.message = error.to_string();
        }),
    }
    registry::emit_job(app, state, job_id);
    if let Err(error) = registry::persist(app, state) {
        log_error!("下载任务状态保存失败：{error}");
    }
}

/// 单个后台下载任务共享的运行时句柄。
#[derive(Clone, Copy)]
pub(super) struct JobContext<'a> {
    /// 用于访问共享状态、事件和文件系统。
    pub(super) app: &'a tauri::AppHandle,
    /// 下载任务注册表。
    pub(super) state: &'a DownloadState,
    /// 当前后台任务的唯一标识。
    pub(super) job_id: &'a str,
    /// 任务取消标记。
    pub(super) cancelled: &'a AtomicBool,
}

/// 启动文字章节下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
/// - `novel_id`：小说编号。
/// - `title`：本地目录与展示名称。
/// - `chapter_ids`：待下载章节编号。
/// - `cancelled`：任务取消标记。
pub(super) fn spawn_text(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
    title: String,
    chapter_ids: Vec<i64>,
    cancelled: Arc<AtomicBool>,
) {
    spawn_with_slot(text::run(
        app,
        job_id,
        novel_id,
        title,
        chapter_ids,
        cancelled,
    ));
}

/// 启动有声章节下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
/// - `novel_id`：小说编号。
/// - `album_id`：可选的有声专辑编号。
/// - `title`：本地目录与展示名称。
/// - `chapter_ids`：待下载章节编号。
/// - `cancelled`：任务取消标记。
pub(super) fn spawn_audio(
    app: tauri::AppHandle,
    job_id: String,
    novel_id: i64,
    album_id: Option<i64>,
    title: String,
    chapter_ids: Vec<i64>,
    cancelled: Arc<AtomicBool>,
) {
    spawn_with_slot(audio::run(
        app,
        job_id,
        novel_id,
        album_id,
        title,
        chapter_ids,
        cancelled,
    ));
}

/// 启动漫画章节下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
/// - `comic_id`：漫画编号。
/// - `source_path`：可选的网页来源路径。
/// - `title`：本地目录与展示名称。
/// - `chapter_ids`：待下载章节编号。
/// - `cancelled`：任务取消标记。
pub(super) fn spawn_comic(
    app: tauri::AppHandle,
    job_id: String,
    comic_id: i64,
    source_path: Option<String>,
    title: String,
    chapter_ids: Vec<i64>,
    cancelled: Arc<AtomicBool>,
) {
    spawn_with_slot(comic::run(
        app,
        job_id,
        comic_id,
        source_path,
        title,
        chapter_ids,
        cancelled,
    ));
}

/// 根据持久化参数重新分派下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
/// - `spec`：任务类型、来源和章节参数。
/// - `cancelled`：新的任务取消标记。
pub(super) fn spawn_from_spec(
    app: tauri::AppHandle,
    job_id: String,
    spec: DownloadSpec,
    cancelled: Arc<AtomicBool>,
) {
    match spec.kind {
        MediaKind::Audio => spawn_audio(
            app,
            job_id,
            spec.novel_id,
            spec.source_id,
            spec.title,
            spec.chapter_ids,
            cancelled,
        ),
        MediaKind::Comic => spawn_comic(
            app,
            job_id,
            spec.source_id.unwrap_or(spec.novel_id),
            spec.source_path,
            spec.title,
            spec.chapter_ids,
            cancelled,
        ),
        MediaKind::Text => spawn_text(
            app,
            job_id,
            spec.novel_id,
            spec.title,
            spec.chapter_ids,
            cancelled,
        ),
    }
}
