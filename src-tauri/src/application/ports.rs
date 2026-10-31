//! 应用层依赖的端口。实现位于适配器层。

use crate::domain::auth::AuthSession;
use crate::domain::auth::UserProfile;
use crate::domain::catalog::{
    AudioChapter, ChapterVolume, ComicChapter, NovelDetail, PublicShelf, SearchNovel,
};
use crate::domain::content::{
    ApiChapterContent, OnlineAudioTrack, OnlineComicChapter, OnlineTextChapter,
};
use crate::domain::download::DownloadJob;
use crate::domain::error::AppResult;
use crate::domain::library::{
    BookDetail, ExportFile, LibraryComicChapter, LibraryEntry, StoredChapter,
};
use crate::domain::settings::{ContentDictionaryResult, RequestPolicy};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

/// 远程 SF 内容端口。
#[async_trait]
pub(crate) trait RemoteGateway: Send + Sync {
    /// 搜索公开作品。
    async fn search(&self, query: &str) -> AppResult<Vec<SearchNovel>>;
    /// 读取小说详情。
    async fn novel_detail(&self, novel_id: i64) -> AppResult<NovelDetail>;
    /// 读取有声专辑对应的作品详情。
    async fn audio_detail(&self, album_id: i64, novel_id: i64) -> AppResult<NovelDetail>;
    /// 读取漫画作品详情。
    async fn comic_detail(&self, comic_id: i64) -> AppResult<NovelDetail>;
    /// 读取文字分卷与章节。
    async fn text_volumes(&self, novel_id: i64) -> AppResult<Vec<ChapterVolume>>;
    /// 解析作品对应的有声专辑编号。
    async fn audio_album_id(&self, novel_id: i64, title_hint: Option<&str>) -> AppResult<i64>;
    /// 读取有声专辑标题与章节。
    async fn audio_catalog(
        &self,
        album_id: i64,
        title_hint: Option<&str>,
    ) -> AppResult<(String, Vec<AudioChapter>)>;
    /// 解析漫画标题和来源标识。
    async fn comic_source(
        &self,
        comic_id: i64,
        source_path: Option<&str>,
        title_hint: Option<&str>,
    ) -> AppResult<(String, String)>;
    /// 读取漫画 App 章节。
    async fn comic_catalog(&self, comic_id: i64) -> AppResult<Vec<ComicChapter>>;
    /// 读取当前账号的公开书架。
    async fn bookshelf(&self, force_refresh: bool) -> AppResult<PublicShelf>;
    /// 读取当前账号资料。
    async fn account_profile(&self) -> AppResult<UserProfile>;
    /// 通过上游请求验证当前账号会话。
    async fn verify_account(&self) -> AppResult<()>;
    /// 使用账号密码换取 App Cookie。
    async fn login_app(&self, username: &str, password: &str) -> AppResult<String>;
    /// 使用指定 App 会话注册当前设备。
    async fn register_device(&self, app_cookie: &str) -> AppResult<()>;
    /// 读取 App 章节正文及对齐所需标识。
    async fn app_chapter_content(&self, chapter_id: i64) -> AppResult<ApiChapterContent>;
    /// 读取网页章节正文。
    async fn web_chapter_content(
        &self,
        novel_id: i64,
        volume_id: i64,
        chapter_id: i64,
    ) -> AppResult<String>;
}

/// 会话同步与平台持久化端口。
#[async_trait]
pub(crate) trait AuthGateway: Send + Sync {
    /// 从平台持久化层同步会话。
    async fn synchronize(&self) -> AppResult<Option<AuthSession>>;
    /// 持久化 App Cookie 和账号名称。
    async fn persist_app_session(&self, cookie: &str, user_name: &str) -> AppResult<()>;
    /// 持久化 Web Cookie。
    async fn persist_web_session(&self, cookie: &str) -> AppResult<()>;
    /// 启动平台官方登录流程。
    async fn start_official_login(&self) -> AppResult<Option<AuthSession>>;
    /// 清除全部会话与持久化数据。
    async fn clear_all(&self) -> AppResult<()>;
    /// 仅清除 App 会话。
    async fn clear_app(&self) -> AppResult<()>;
    /// 仅清除 Web 会话。
    async fn clear_web(&self) -> AppResult<()>;
}

/// 本地书库端口。
#[async_trait]
pub(crate) trait LibraryStore: Send + Sync {
    /// 列出本地书籍摘要。
    fn list(&self) -> AppResult<Vec<LibraryEntry>>;
    /// 读取本地书籍详情。
    fn get_book(&self, name: &str) -> AppResult<BookDetail>;
    /// 读取本地文字章节。
    fn get_chapter(&self, name: &str, chapter_id: i64) -> AppResult<StoredChapter>;
    /// 读取本地漫画章节。
    fn get_comic_chapter(&self, name: &str, chapter_id: i64) -> AppResult<LibraryComicChapter>;
    /// 删除本地书籍。
    fn delete(&self, name: &str) -> AppResult<()>;
    /// 读取某类媒体的已下载章节编号。
    fn downloaded_ids(&self, kind: &str, title: Option<&str>) -> AppResult<HashSet<i64>>;
    /// 打开下载目录。
    async fn open_download_directory(&self) -> AppResult<()>;
    /// 确保外部存储权限可用。
    async fn ensure_external_storage_access(&self) -> AppResult<bool>;
    /// 导出本地书籍。
    async fn export(&self, name: &str, format: &str, output_path: &str) -> AppResult<ExportFile>;
}

/// 用户设置与正文恢复字典端口。
#[async_trait]
pub(crate) trait SettingsStore: Send + Sync {
    /// 读取请求策略。
    fn load_policy(&self) -> AppResult<RequestPolicy>;
    /// 保存请求策略。
    fn save_policy(&self, policy: &RequestPolicy) -> AppResult<()>;
    /// 读取正文恢复字典。
    fn load_dictionary(&self) -> AppResult<HashMap<String, String>>;
    /// 覆盖写入正文恢复字典。
    fn write_dictionary(&self, dictionary: &HashMap<String, String>) -> AppResult<()>;
    /// 返回字典条目数量。
    fn dictionary_size(&self) -> AppResult<ContentDictionaryResult>;
}

/// 下载任务端口。
#[async_trait]
pub(crate) trait DownloadGateway: Send + Sync {
    /// 创建文字下载任务。
    async fn create_text(
        &self,
        novel_id: i64,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<DownloadJob>;
    /// 创建有声下载任务。
    async fn create_audio(
        &self,
        novel_id: i64,
        album_id: Option<i64>,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<DownloadJob>;
    /// 创建漫画下载任务。
    async fn create_comic(
        &self,
        comic_id: i64,
        source_path: Option<String>,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<DownloadJob>;
    /// 列出全部下载任务。
    fn list(&self) -> AppResult<Vec<DownloadJob>>;
    /// 暂停下载任务。
    fn pause(&self, job_id: &str) -> AppResult<DownloadJob>;
    /// 恢复下载任务。
    fn resume(&self, job_id: &str) -> AppResult<DownloadJob>;
    /// 删除下载任务。
    fn delete(&self, job_id: &str) -> AppResult<()>;
    /// 从持久化快照恢复任务列表。
    fn restore(&self) -> AppResult<()>;
}

/// 在线阅读与媒体协议端口。
#[async_trait]
pub(crate) trait OnlineReader: Send + Sync {
    /// 读取在线文字章节。
    async fn load_text(&self, novel_id: i64, chapter_id: i64) -> AppResult<OnlineTextChapter>;
    /// 读取在线漫画章节。
    async fn load_comic(
        &self,
        comic_id: i64,
        chapter_id: i64,
        chapter_title: &str,
        source_path: Option<&str>,
    ) -> AppResult<OnlineComicChapter>;
    /// 读取在线音频流地址。
    async fn load_audio(
        &self,
        chapter_id: i64,
        chapter_title: &str,
        chapter_volume: &str,
    ) -> AppResult<OnlineAudioTrack>;
}
