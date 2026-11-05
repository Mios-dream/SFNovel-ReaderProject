//! 远程目录、详情和书架命令。

use crate::application::catalog::CatalogService;
use crate::domain::catalog::{
    AudioCatalog, BookCommentPage, BookCommentReplyPage, BookshelfCollection, ChapterVolume,
    ComicCatalog, DiscoverFeed, NovelDetail, SearchNovel,
};
use crate::domain::error::AppResult;
use crate::domain::ids::NovelId;

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
) -> AppResult<Vec<SearchNovel>> {
    let (remote, storage) = super::runtime(&app).await?;
    CatalogService::new(&remote, &storage).search(&query).await
}

#[tauri::command]
/// 读取首页推荐、分类、排行和最近更新。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `type_id`：可选的分类编号。
///
/// # 错误
/// 分类编号非法、初始化失败或上游请求失败时返回错误消息。
pub(crate) async fn get_discover_feed(
    app: tauri::AppHandle,
    type_id: Option<i64>,
) -> AppResult<DiscoverFeed> {
    let (remote, storage) = super::runtime(&app).await?;
    CatalogService::new(&remote, &storage)
        .discover(type_id)
        .await
}

#[tauri::command]
/// 读取小说、漫画或有声作品的公开评论。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `kind`：`novel`、`audio` 或 `comic`。
/// - `novel_id`：小说编号；漫画使用漫画编号。
/// - `media_id`：有声专辑编号。
/// - `page`：从 0 开始的页码。
/// - `size`：单页评论数量。
/// - `reviews`：true 时读取小说书评，否则读取普通评论。
///
/// # 错误
/// 类型或编号非法、初始化失败或上游请求失败时返回错误消息。
pub(crate) async fn get_book_comments(
    app: tauri::AppHandle,
    kind: String,
    novel_id: i64,
    media_id: Option<i64>,
    page: i64,
    size: i64,
    reviews: bool,
) -> AppResult<BookCommentPage> {
    let (remote, storage) = super::runtime(&app).await?;
    CatalogService::new(&remote, &storage)
        .book_comments(&kind, novel_id, media_id, page, size, reviews)
        .await
}

#[tauri::command]
/// 读取一条评论的回复。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `kind`：`novel`、`audio` 或 `comic`。
/// - `comment_id`：评论编号。
/// - `reviews`：true 时读取小说书评回复，否则读取普通评论回复。
/// - `page`：从 0 开始的页码。
/// - `size`：单页回复数量。
///
/// # 错误
/// 类型或编号非法、初始化失败或上游请求失败时返回错误消息。
pub(crate) async fn get_comment_replies(
    app: tauri::AppHandle,
    kind: String,
    comment_id: i64,
    reviews: bool,
    page: i64,
    size: i64,
) -> AppResult<BookCommentReplyPage> {
    let (remote, storage) = super::runtime(&app).await?;
    CatalogService::new(&remote, &storage)
        .comment_replies(&kind, comment_id, reviews, page, size)
        .await
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
) -> AppResult<NovelDetail> {
    let (remote, storage) = super::runtime(&app).await?;
    let novel_id = NovelId::new(novel_id)?;
    CatalogService::new(&remote, &storage)
        .novel_detail(novel_id)
        .await
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
) -> AppResult<NovelDetail> {
    let (remote, storage) = super::runtime(&app).await?;
    let novel_id = NovelId::new(novel_id)?;
    CatalogService::new(&remote, &storage)
        .audio_detail(album_id, novel_id)
        .await
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
) -> AppResult<Vec<ChapterVolume>> {
    let (remote, storage) = super::runtime(&app).await?;
    let novel_id = NovelId::new(novel_id)?;
    CatalogService::new(&remote, &storage)
        .chapter_volumes(novel_id)
        .await
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
) -> AppResult<AudioCatalog> {
    let (remote, storage) = super::runtime(&app).await?;
    let novel_id = NovelId::new(novel_id)?;
    CatalogService::new(&remote, &storage)
        .audio_catalog(novel_id, album_id, title_hint)
        .await
}

#[tauri::command]
/// 读取漫画章节目录。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `comic_id`：漫画编号。
/// - `source_path`：可选的来源路径，用于读取作品标题。
/// - `title_hint`：用于生成标题回退值的提示。
///
/// # 错误
/// 编号非法或上游请求失败时返回错误消息。
pub(crate) async fn get_comic_chapters(
    app: tauri::AppHandle,
    comic_id: i64,
    source_path: Option<String>,
    title_hint: Option<String>,
) -> AppResult<ComicCatalog> {
    let (remote, storage) = super::runtime(&app).await?;
    CatalogService::new(&remote, &storage)
        .comic_catalog(comic_id, source_path, title_hint)
        .await
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
) -> AppResult<NovelDetail> {
    let (remote, storage) = super::runtime(&app).await?;
    CatalogService::new(&remote, &storage)
        .comic_detail(comic_id)
        .await
}

#[tauri::command]
/// 读取当前登录账号的公开书架。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `force_refresh`：为 true 时跳过缓存重新请求书架。
///
/// # 错误
/// 未登录或上游书架请求失败时返回错误消息。
pub(crate) async fn get_bookshelf(
    app: tauri::AppHandle,
    force_refresh: Option<bool>,
) -> AppResult<BookshelfCollection> {
    let (remote, storage) = super::runtime(&app).await?;
    CatalogService::new(&remote, &storage)
        .bookshelf(force_refresh)
        .await
}

#[tauri::command]
/// 收藏小说、有声专辑或漫画。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `kind`：`novel`、`audio` 或 `comic`。
/// - `novel_id`：小说编号；漫画使用漫画编号。
/// - `media_id`：有声专辑编号。
///
/// # 错误
/// 类型不支持、编号非法、未登录 App 或上游请求失败时返回错误消息。
pub(crate) async fn add_remote_favorite(
    app: tauri::AppHandle,
    kind: String,
    novel_id: i64,
    media_id: Option<i64>,
) -> AppResult<()> {
    let (remote, storage) = super::runtime(&app).await?;
    CatalogService::new(&remote, &storage)
        .add_favorite(&kind, novel_id, media_id)
        .await
}

#[tauri::command]
/// 取消小说、有声专辑或漫画收藏。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `kind`：`novel`、`audio` 或 `comic`。
/// - `novel_id`：小说编号；漫画使用漫画编号。
/// - `media_id`：有声专辑编号。
///
/// # 错误
/// 类型不支持、编号非法、未登录 App 或上游请求失败时返回错误消息。
pub(crate) async fn remove_remote_favorite(
    app: tauri::AppHandle,
    kind: String,
    novel_id: i64,
    media_id: Option<i64>,
) -> AppResult<()> {
    let (remote, storage) = super::runtime(&app).await?;
    CatalogService::new(&remote, &storage)
        .remove_favorite(&kind, novel_id, media_id)
        .await
}
