//! 下载任务的后台执行器。
//!
//! 本模块只负责把持久化任务分派给对应媒体流程；任务状态和进度发布统一由
//! 上级注册表处理。

mod assets;
mod audio;
mod comic;
mod text;

use crate::adapters::downloads::DownloadState;
use crate::domain::download::DownloadSpec;
use std::future::Future;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Semaphore;

/// 全进程共享的下载并发槽位。
static DOWNLOAD_SLOTS: OnceLock<Mutex<Arc<Semaphore>>> = OnceLock::new();

/// 更新下载任务并发上限。
pub(crate) fn configure_concurrency(max_concurrent: u8) {
    let permits = usize::from(max_concurrent.clamp(1, 4));
    let slots = Arc::new(Semaphore::new(permits));
    match DOWNLOAD_SLOTS.get() {
        Some(current) => {
            if let Ok(mut guard) = current.lock() {
                *guard = slots;
            }
        }
        None => {
            let _ = DOWNLOAD_SLOTS.set(Mutex::new(slots));
        }
    }
}

/// 等待并取得一个下载并发槽位。
async fn acquire_slot() -> tokio::sync::OwnedSemaphorePermit {
    let slots = DOWNLOAD_SLOTS
        .get_or_init(|| Mutex::new(Arc::new(Semaphore::new(1))))
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| Arc::new(Semaphore::new(1)));
    slots.acquire_owned().await.expect("下载并发槽位不应关闭")
}

/// 在取得并发槽位后执行单个下载任务。
fn spawn_with_slot<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let _slot = acquire_slot().await;
        future.await;
    });
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
    match spec.kind.as_str() {
        "audio" => spawn_audio(
            app,
            job_id,
            spec.novel_id,
            spec.source_id,
            spec.title,
            spec.chapter_ids,
            cancelled,
        ),
        "comic" => spawn_comic(
            app,
            job_id,
            spec.source_id.unwrap_or(spec.novel_id),
            spec.source_path,
            spec.title,
            spec.chapter_ids,
            cancelled,
        ),
        _ => spawn_text(
            app,
            job_id,
            spec.novel_id,
            spec.title,
            spec.chapter_ids,
            cancelled,
        ),
    }
}
