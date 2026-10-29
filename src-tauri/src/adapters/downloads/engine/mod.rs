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
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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
    tauri::async_runtime::spawn(text::run(
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
    tauri::async_runtime::spawn(audio::run(
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
    tauri::async_runtime::spawn(comic::run(
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
