//! 远程目录、详情和书架命令。

use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::NativeStorage;
use crate::application::catalog::CatalogService;
use crate::domain::catalog::{
    AudioCatalog, BookshelfCollection, ChapterVolume, ComicCatalog, NovelDetail, SearchNovel,
};
use crate::domain::error::AppError;

/// 组装目录命令所需的远程来源和本地书库。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// SF 内容源初始化失败时返回错误。
async fn service_parts(app: &tauri::AppHandle) -> Result<(SfSource, NativeStorage), AppError> {
    let remote = SfSource::connect(app).await.map_err(AppError::upstream)?;
    Ok((remote, NativeStorage::new(app.clone())))
}

#[tauri::command]
/// 搜索 SF 作品。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `query`：搜索关键词，最长 100 个字符。
///
/// # 错误
/// 关键词过长、上游请求失败或初始化失败时返回错误消息。
pub(crate) async fn search_novels(
    app: tauri::AppHandle,
    query: String,
) -> Result<Vec<SearchNovel>, String> {
    let (remote, storage) = service_parts(&app).await.map_err(AppError::into_message)?;
    CatalogService::new(&remote, &storage)
        .search(&query)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取小说详情。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `novel_id`：小说编号。
///
/// # 错误
/// 编号非法或上游请求失败时返回错误消息。
pub(crate) async fn get_novel_details(
    app: tauri::AppHandle,
    novel_id: i64,
) -> Result<NovelDetail, String> {
    let (remote, storage) = service_parts(&app).await.map_err(AppError::into_message)?;
    CatalogService::new(&remote, &storage)
        .novel_detail(novel_id)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取有声专辑对应的作品详情。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `album_id`：有声专辑编号。
/// - `novel_id`：所属小说编号。
///
/// # 错误
/// 编号非法或上游请求失败时返回错误消息。
pub(crate) async fn get_audio_details(
    app: tauri::AppHandle,
    album_id: i64,
    novel_id: i64,
) -> Result<NovelDetail, String> {
    let (remote, storage) = service_parts(&app).await.map_err(AppError::into_message)?;
    CatalogService::new(&remote, &storage)
        .audio_detail(album_id, novel_id)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取文字小说的分卷与章节目录。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `novel_id`：小说编号。
///
/// # 错误
/// 编号非法、没有可用章节或上游请求失败时返回错误消息。
pub(crate) async fn get_chapter_volumes(
    app: tauri::AppHandle,
    novel_id: i64,
) -> Result<Vec<ChapterVolume>, String> {
    let (remote, storage) = service_parts(&app).await.map_err(AppError::into_message)?;
    CatalogService::new(&remote, &storage)
        .chapter_volumes(novel_id)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取有声章节目录。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `novel_id`：小说编号。
/// - `album_id`：可选的有声专辑编号。
/// - `title_hint`：缺少专辑编号时用于解析专辑的标题提示。
///
/// # 错误
/// 编号非法、专辑无法解析或上游请求失败时返回错误消息。
pub(crate) async fn get_audio_chapters(
    app: tauri::AppHandle,
    novel_id: i64,
    album_id: Option<i64>,
    title_hint: Option<String>,
) -> Result<AudioCatalog, String> {
    let (remote, storage) = service_parts(&app).await.map_err(AppError::into_message)?;
    CatalogService::new(&remote, &storage)
        .audio_catalog(novel_id, album_id, title_hint)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取漫画章节目录。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `comic_id`：漫画编号。
/// - `source_path`：可选的网页来源路径。
/// - `title_hint`：用于解析漫画来源目录的标题提示。
///
/// # 错误
/// 编号非法、来源目录无法解析或上游请求失败时返回错误消息。
pub(crate) async fn get_comic_chapters(
    app: tauri::AppHandle,
    comic_id: i64,
    source_path: Option<String>,
    title_hint: Option<String>,
) -> Result<ComicCatalog, String> {
    let (remote, storage) = service_parts(&app).await.map_err(AppError::into_message)?;
    CatalogService::new(&remote, &storage)
        .comic_catalog(comic_id, source_path, title_hint)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取漫画作品详情。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `comic_id`：漫画编号。
///
/// # 错误
/// 编号非法或上游请求失败时返回错误消息。
pub(crate) async fn get_comic_details(
    app: tauri::AppHandle,
    comic_id: i64,
) -> Result<NovelDetail, String> {
    let (remote, storage) = service_parts(&app).await.map_err(AppError::into_message)?;
    CatalogService::new(&remote, &storage)
        .comic_detail(comic_id)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取当前登录账号的公开书架。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `force_refresh`：调用方的刷新意图；当前实现始终重新请求。
///
/// # 错误
/// 未登录或上游书架请求失败时返回错误消息。
pub(crate) async fn get_bookshelf(
    app: tauri::AppHandle,
    force_refresh: Option<bool>,
) -> Result<BookshelfCollection, String> {
    let (remote, storage) = service_parts(&app).await.map_err(AppError::into_message)?;
    CatalogService::new(&remote, &storage)
        .bookshelf(force_refresh)
        .await
        .map_err(AppError::into_message)
}
