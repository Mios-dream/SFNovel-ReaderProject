//! 本地书库与导出命令。

use crate::adapters::sfacg::source::SfSource;
use crate::adapters::storage::NativeStorage;
use crate::application::library::LibraryService;
use crate::application::settings::SettingsService;
use crate::domain::error::AppError;
use crate::domain::library::{
    BookDetail, ExportFile, LibraryComicChapter, LibraryEntry, StoredChapter,
};
use crate::domain::settings::ContentDictionaryResult;

#[tauri::command]
/// 列出本地书库中的全部书籍。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 书库目录无法读取时返回错误消息。
pub(crate) fn list_local_library(app: tauri::AppHandle) -> Result<Vec<LibraryEntry>, String> {
    let storage = NativeStorage::new(app);
    LibraryService::new(&storage)
        .list()
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 在系统文件管理器中打开下载目录。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 目录无法创建或系统打开操作失败时返回错误消息。
pub(crate) async fn open_download_directory(app: tauri::AppHandle) -> Result<(), String> {
    let storage = NativeStorage::new(app);
    LibraryService::new(&storage)
        .open_download_directory()
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取一本本地书籍的详情。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `name`：本地书籍目录名。
///
/// # 错误
/// 书籍不存在或清单无法解析时返回错误消息。
pub(crate) fn get_local_book(app: tauri::AppHandle, name: String) -> Result<BookDetail, String> {
    let storage = NativeStorage::new(app);
    LibraryService::new(&storage)
        .book(&name)
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取本地文字章节。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `name`：本地书籍目录名。
/// - `chapter_id`：章节编号。
///
/// # 错误
/// 编号非法、书籍或章节不存在时返回错误消息。
pub(crate) fn get_local_chapter(
    app: tauri::AppHandle,
    name: String,
    chapter_id: i64,
) -> Result<StoredChapter, String> {
    let storage = NativeStorage::new(app);
    LibraryService::new(&storage)
        .chapter(&name, chapter_id)
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 读取本地漫画章节及其页面列表。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `name`：本地书籍目录名。
/// - `chapter_id`：漫画章节编号。
///
/// # 错误
/// 编号非法、书籍或章节不存在时返回错误消息。
pub(crate) fn get_local_comic_chapter(
    app: tauri::AppHandle,
    name: String,
    chapter_id: i64,
) -> Result<LibraryComicChapter, String> {
    let storage = NativeStorage::new(app);
    LibraryService::new(&storage)
        .comic_chapter(&name, chapter_id)
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 删除一本本地书籍及其全部文件。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `name`：本地书籍目录名。
///
/// # 错误
/// 书籍不存在或文件系统删除失败时返回错误消息。
pub(crate) fn delete_local_book(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let storage = NativeStorage::new(app);
    LibraryService::new(&storage)
        .delete(&name)
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 请求或确认外部存储访问权限。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 返回
/// 当前是否已获得可用权限。
///
/// # 错误
/// 平台权限查询失败时返回错误消息。
pub(crate) async fn ensure_external_storage_access(app: tauri::AppHandle) -> Result<bool, String> {
    let storage = NativeStorage::new(app);
    LibraryService::new(&storage)
        .ensure_external_storage_access()
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 导出一本本地书籍。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `name`：本地书籍目录名。
/// - `format`：`epub`、`markdown`、`txt`、`audio` 或 `comic`。
/// - `output_path`：导出目标路径。
///
/// # 错误
/// 格式不支持、书籍不存在或导出写入失败时返回错误消息。
pub(crate) async fn export_local_book(
    app: tauri::AppHandle,
    name: String,
    format: String,
    output_path: String,
) -> Result<ExportFile, String> {
    let storage = NativeStorage::new(app);
    LibraryService::new(&storage)
        .export(&name, &format, &output_path)
        .await
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 返回正文恢复字典的当前规模。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
///
/// # 错误
/// 字典文件无法读取时返回错误消息。
pub(crate) fn get_content_dictionary(
    app: tauri::AppHandle,
) -> Result<ContentDictionaryResult, String> {
    let storage = NativeStorage::new(app);
    SettingsService::new(&storage)
        .content_dictionary()
        .map_err(AppError::into_message)
}

#[tauri::command]
/// 使用指定章节的 API 与网页正文更新恢复字典。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `chapter_id`：用于正文比对的文字章节编号。
///
/// # 错误
/// 上游读取失败、正文无法对齐、映射冲突或字典写入失败时返回错误消息。
pub(crate) async fn update_content_dictionary(
    app: tauri::AppHandle,
    chapter_id: i64,
) -> Result<ContentDictionaryResult, String> {
    let remote = SfSource::connect(&app)
        .await
        .map_err(AppError::upstream)
        .map_err(AppError::into_message)?;
    let storage = NativeStorage::new(app);
    SettingsService::new(&storage)
        .update_content_dictionary(&remote, chapter_id)
        .await
        .map_err(AppError::into_message)
}
