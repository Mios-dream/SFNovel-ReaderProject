//! 本地书库用例。

use crate::application::ports::LibraryStore;
use crate::domain::error::{AppError, AppResult};
use crate::domain::library::{
    BookDetail, ExportFile, LibraryComicChapter, LibraryEntry, StoredChapter,
};

/// 对本地书库操作执行参数校验的应用服务。
pub(crate) struct LibraryService<'a, S: LibraryStore> {
    /// 本地书库及导出实现。
    store: &'a S,
}

impl<'a, S: LibraryStore> LibraryService<'a, S> {
    /// 创建本地书库服务。
    ///
    /// # 参数
    /// - `store`：本地书库实现。
    pub(crate) fn new(store: &'a S) -> Self {
        Self { store }
    }

    /// 列出本地书籍摘要。
    ///
    /// # 错误
    /// 书库目录无法读取时返回错误。
    pub(crate) fn list(&self) -> AppResult<Vec<LibraryEntry>> {
        self.store.list()
    }

    /// 读取指定本地书籍详情。
    ///
    /// # 参数
    /// - `name`：本地书籍目录名。
    ///
    /// # 错误
    /// 书籍不存在或清单无法解析时返回错误。
    pub(crate) fn book(&self, name: &str) -> AppResult<BookDetail> {
        self.store.get_book(name)
    }

    /// 读取一本书的本地文字章节。
    ///
    /// # 参数
    /// - `name`：本地书籍目录名。
    /// - `chapter_id`：正整数章节编号。
    ///
    /// # 错误
    /// 章节编号非法、书籍不存在或章节文件无法解析时返回错误。
    pub(crate) fn chapter(&self, name: &str, chapter_id: i64) -> AppResult<StoredChapter> {
        if chapter_id <= 0 {
            return Err(AppError::invalid_input("章节参数无效"));
        }
        self.store.get_chapter(name, chapter_id)
    }

    /// 读取一本书的本地漫画章节。
    ///
    /// # 参数
    /// - `name`：本地书籍目录名。
    /// - `chapter_id`：正整数章节编号。
    ///
    /// # 错误
    /// 章节编号非法、书籍不存在或章节文件无法解析时返回错误。
    pub(crate) fn comic_chapter(
        &self,
        name: &str,
        chapter_id: i64,
    ) -> AppResult<LibraryComicChapter> {
        if chapter_id <= 0 {
            return Err(AppError::invalid_input("漫画章节参数无效"));
        }
        self.store.get_comic_chapter(name, chapter_id)
    }

    /// 删除一本本地书籍。
    ///
    /// # 参数
    /// - `name`：本地书籍目录名。
    ///
    /// # 错误
    /// 书籍不存在或文件系统删除失败时返回错误。
    pub(crate) fn delete(&self, name: &str) -> AppResult<()> {
        self.store.delete(name)
    }

    /// 请求或确认外部存储访问权限。
    ///
    /// # 返回
    /// 当前是否已获得可用权限。
    ///
    /// # 错误
    /// 平台权限查询失败时返回错误。
    pub(crate) async fn ensure_external_storage_access(&self) -> AppResult<bool> {
        self.store.ensure_external_storage_access().await
    }

    /// 在系统文件管理器中打开下载目录。
    ///
    /// # 错误
    /// 下载目录无法创建或系统打开操作失败时返回错误。
    pub(crate) async fn open_download_directory(&self) -> AppResult<()> {
        self.store.open_download_directory().await
    }

    /// 将本地书籍导出为指定格式。
    ///
    /// # 参数
    /// - `name`：本地书籍目录名。
    /// - `format`：`epub`、`markdown`、`txt`、`audio` 或 `comic`。
    /// - `output_path`：导出文件或目录的目标路径。
    ///
    /// # 错误
    /// 格式不受支持、书籍不存在或导出写入失败时返回错误。
    pub(crate) async fn export(
        &self,
        name: &str,
        format: &str,
        output_path: &str,
    ) -> AppResult<ExportFile> {
        if !matches!(format, "epub" | "markdown" | "txt" | "audio" | "comic") {
            return Err(AppError::invalid_input("不支持的导出格式"));
        }
        self.store.export(name, format, output_path).await
    }
}
