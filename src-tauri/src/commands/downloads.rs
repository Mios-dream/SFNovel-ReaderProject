//! 下载队列命令。

use crate::adapters::downloads::NativeDownloadGateway;
use crate::application::downloads::DownloadService;
use crate::domain::download::DownloadJob;
use crate::domain::error::AppError;

#[tauri::command]
/// 创建文字小说下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `novel_id`：小说编号。
/// - `title`：任务与本地目录名称。
/// - `chapter_ids`：待下载章节编号。
///
/// # 错误
/// 编号、标题或章节选择非法，或下载队列拒绝创建任务时返回错误消息。
pub(crate) async fn create_text_download(
    app: tauri::AppHandle,
    novel_id: i64,
    title: String,
    chapter_ids: Vec<i64>,
) -> Result<DownloadJob, String> {
    let gateway = NativeDownloadGateway::new(app);
    DownloadService::new(&gateway)
        .create_text(novel_id, title, chapter_ids)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 创建有声下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `novel_id`：小说编号。
/// - `album_id`：可选的有声专辑编号。
/// - `title`：任务与本地目录名称。
/// - `chapter_ids`：待下载章节编号。
///
/// # 错误
/// 编号、标题或章节选择非法，或下载队列拒绝创建任务时返回错误消息。
pub(crate) async fn create_audio_download(
    app: tauri::AppHandle,
    novel_id: i64,
    album_id: Option<i64>,
    title: String,
    chapter_ids: Vec<i64>,
) -> Result<DownloadJob, String> {
    let gateway = NativeDownloadGateway::new(app);
    DownloadService::new(&gateway)
        .create_audio(novel_id, album_id, title, chapter_ids)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 创建漫画下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `comic_id`：漫画编号。
/// - `source_path`：可选的网页来源路径。
/// - `title`：任务与本地目录名称。
/// - `chapter_ids`：待下载章节编号。
///
/// # 错误
/// 编号、标题或章节选择非法，或下载队列拒绝创建任务时返回错误消息。
pub(crate) async fn create_comic_download(
    app: tauri::AppHandle,
    comic_id: i64,
    source_path: Option<String>,
    title: String,
    chapter_ids: Vec<i64>,
) -> Result<DownloadJob, String> {
    let gateway = NativeDownloadGateway::new(app);
    DownloadService::new(&gateway)
        .create_comic(comic_id, source_path, title, chapter_ids)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 列出当前全部下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 下载队列状态不可用时返回错误消息。
pub(crate) fn list_download_jobs(app: tauri::AppHandle) -> Result<Vec<DownloadJob>, String> {
    let gateway = NativeDownloadGateway::new(app);
    DownloadService::new(&gateway)
        .list()
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 暂停指定下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
///
/// # 错误
/// 任务不存在或当前状态无法暂停时返回错误消息。
pub(crate) fn pause_download_job(
    app: tauri::AppHandle,
    job_id: String,
) -> Result<DownloadJob, String> {
    let gateway = NativeDownloadGateway::new(app);
    DownloadService::new(&gateway)
        .pause(&job_id)
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 恢复指定下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
///
/// # 错误
/// 任务不存在、缺少恢复参数或当前状态无法恢复时返回错误消息。
pub(crate) fn resume_download_job(
    app: tauri::AppHandle,
    job_id: String,
) -> Result<DownloadJob, String> {
    let gateway = NativeDownloadGateway::new(app);
    DownloadService::new(&gateway)
        .resume(&job_id)
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 删除指定下载任务。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
///
/// # 错误
/// 任务不存在或队列状态更新失败时返回错误消息。
pub(crate) fn delete_download_job(app: tauri::AppHandle, job_id: String) -> Result<(), String> {
    let gateway = NativeDownloadGateway::new(app);
    DownloadService::new(&gateway)
        .delete(&job_id)
        .map_err(AppError::into_message)
}
