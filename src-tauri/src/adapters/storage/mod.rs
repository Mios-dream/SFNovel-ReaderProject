//! 本地文件、书库、设置与下载快照适配器。

mod atomic;
mod book;
mod export;
mod jobs;
pub(crate) mod paths;
mod settings;
#[cfg(test)]
pub(crate) mod test_support;

use crate::application::ports::{LibraryStore, SettingsStore};
use crate::domain::error::{AppError, AppResult};
use async_trait::async_trait;
use tauri::AppHandle;

pub(crate) use atomic::replace_file;
pub(crate) use book::{
    downloaded_text_chapter_ids, read_book_manifest, read_chapter_store, write_book_manifest,
    write_chapter_store,
};
pub(crate) use jobs::{read_job_snapshot, write_job_snapshot};
pub(crate) use paths::safe_library_name;

/// 面向应用自有目录的原生存储入口。
pub(crate) struct NativeStorage {
    /// 用于解析应用目录和平台权限的句柄。
    app: AppHandle,
}

impl NativeStorage {
    /// 创建原生存储入口。
    ///
    /// # 参数
    /// - `app`：Tauri 应用句柄。
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }

    /// 返回当前配置对应的书库根目录，并确保目录存在。
    ///
    /// # 错误
    /// 应用目录无法解析或书库目录无法创建时返回错误消息。
    pub(crate) fn library_directory(&self) -> AppResult<std::path::PathBuf> {
        paths::library_directory(&self.app)
    }

    /// 创建以安全文件名命名的本地书籍目录。
    ///
    /// # 参数
    /// - `title`：用户可见书名；写入文件系统前会被清理。
    ///
    /// # 错误
    /// 书库目录无法解析或书籍目录无法创建时返回错误消息。
    pub(crate) fn new_book_directory(&self, title: &str) -> AppResult<std::path::PathBuf> {
        let directory = self.library_directory()?.join(safe_library_name(title));
        std::fs::create_dir_all(&directory)
            .map_err(|error| AppError::storage(format!("无法创建本地书籍目录：{error}")))?;
        Ok(directory)
    }

    /// 使用正文恢复字典解码 App 返回的章节文本。
    ///
    /// # 参数
    /// - `content`：上游 App 正文。
    ///
    /// # 错误
    /// 字典无法读取时返回错误消息。
    pub(crate) fn decode_api_content(
        &self,
        content: &str,
        is_content_encrypted: bool,
    ) -> AppResult<String> {
        settings::decode_api_content(&self.app, content, is_content_encrypted)
    }

    /// 读取持久化应用设置。
    ///
    /// # 错误
    /// 设置文件无法读取或解析失败时返回错误消息。
    pub(crate) fn settings(&self) -> AppResult<crate::domain::settings::AppSettings> {
        settings::load_settings(&self.app)
    }

    /// 请求或确认外部存储访问权限。
    ///
    /// # 错误
    /// 平台权限查询失败时返回错误消息。
    pub(crate) async fn ensure_external_storage_access(&self) -> AppResult<bool> {
        settings::ensure_external_storage_access(&self.app).await
    }
}

#[async_trait]
impl LibraryStore for NativeStorage {
    /// 列出书库中的本地书籍摘要。
    fn list(&self) -> AppResult<Vec<crate::domain::library::LibraryEntry>> {
        book::list(&self.app)
    }

    /// 读取指定本地书籍详情。
    fn get_book(&self, name: &str) -> AppResult<crate::domain::library::BookDetail> {
        book::detail(&self.app, name)
    }

    /// 读取本地文字章节。
    fn get_chapter(
        &self,
        name: &str,
        chapter_id: i64,
    ) -> AppResult<crate::domain::library::StoredChapter> {
        book::chapter(&self.app, name, chapter_id)
    }

    /// 读取本地漫画章节。
    fn get_comic_chapter(
        &self,
        name: &str,
        chapter_id: i64,
    ) -> AppResult<crate::domain::library::LibraryComicChapter> {
        book::comic_chapter(&self.app, name, chapter_id)
    }

    /// 删除本地书籍目录。
    fn delete(&self, name: &str) -> AppResult<()> {
        book::delete(&self.app, name)
    }

    /// 读取某类媒体的已下载章节编号。
    fn downloaded_ids(
        &self,
        kind: crate::domain::download::MediaKind,
        scope: crate::domain::library::BookScope<'_>,
    ) -> AppResult<std::collections::HashSet<i64>> {
        book::downloaded_ids(&self.app, kind, scope)
    }

    /// 在文件管理器或系统下载界面打开书库目录。
    async fn open_download_directory(&self) -> AppResult<()> {
        #[cfg(target_os = "android")]
        {
            paths::library_directory(&self.app)?;
            if !settings::load_settings(&self.app)
                ?
                .storage
                .use_external_storage
            {
                return Err(crate::domain::error::AppError::permission_denied(
                    "书库当前使用内部存储，无法从文件管理器直接打开",
                ));
            }
            crate::adapters::platform::android::open_download_directory(&self.app)
                .await
                
        }
        #[cfg(not(target_os = "android"))]
        {
            let directory = paths::library_directory(&self.app)?;
            tauri_plugin_opener::open_path(&directory, None::<&str>).map_err(|error| {
                crate::domain::error::AppError::storage(format!("无法打开下载目录：{error}"))
            })
        }
    }

    /// 请求或确认外部存储访问权限。
    async fn ensure_external_storage_access(&self) -> AppResult<bool> {
        settings::ensure_external_storage_access(&self.app)
            .await
            
    }

    /// 将本地书籍导出为指定格式。
    async fn export(
        &self,
        name: &str,
        format: &str,
        output_path: &str,
    ) -> AppResult<crate::domain::library::ExportFile> {
        export::export(&self.app, name, format, output_path)
            .await
            
    }
}

impl SettingsStore for NativeStorage {
    /// 读取应用设置。
    fn load_settings(&self) -> AppResult<crate::domain::settings::AppSettings> {
        settings::load_settings(&self.app)
    }

    /// 保存应用设置。
    fn save_settings(&self, settings: &crate::domain::settings::AppSettings) -> AppResult<()> {
        settings::save_settings(&self.app, settings)
    }

    /// 读取正文恢复字典。
    fn load_dictionary(&self) -> AppResult<std::collections::HashMap<String, String>> {
        settings::load_content_dictionary(&self.app)
    }

    /// 覆盖写入正文恢复字典。
    fn write_dictionary(
        &self,
        dictionary: &std::collections::HashMap<String, String>,
    ) -> AppResult<()> {
        settings::write_content_dictionary(&self.app, dictionary)
    }

    /// 返回正文恢复字典条目数。
    fn dictionary_size(&self) -> AppResult<crate::domain::settings::ContentDictionaryResult> {
        settings::dictionary_size(&self.app)
    }
}

