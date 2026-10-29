//! 远程目录查询与本地完成状态合并。

use crate::application::ports::{LibraryStore, RemoteGateway};
use crate::domain::catalog::{
    AudioCatalog, AudioChapterSummary, BookshelfCollection, ChapterVolume, ComicCatalog,
    ComicChapterSummary, NovelDetail, SearchNovel,
};
use crate::domain::error::{AppError, AppResult};
use crate::domain::ids::validate_novel_id;

/// 组合远程目录与本地完成状态的查询服务。
pub(crate) struct CatalogService<'a, R: RemoteGateway, S: LibraryStore> {
    /// 远程 SF 目录接口。
    remote: &'a R,
    /// 本地书库接口，用于标记已下载章节。
    store: &'a S,
}

impl<'a, R: RemoteGateway, S: LibraryStore> CatalogService<'a, R, S> {
    /// 创建目录查询服务。
    ///
    /// # 参数
    /// - `remote`：远程目录来源。
    /// - `store`：本地书库来源。
    pub(crate) fn new(remote: &'a R, store: &'a S) -> Self {
        Self { remote, store }
    }

    /// 搜索公开作品。
    ///
    /// # 参数
    /// - `query`：搜索关键词；去除空白后为空时返回空列表。
    ///
    /// # 错误
    /// 关键词超过 100 个字符或上游搜索失败时返回错误。
    pub(crate) async fn search(&self, query: &str) -> AppResult<Vec<SearchNovel>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        if query.chars().count() > 100 {
            return Err(AppError::invalid_input("搜索关键词不能超过 100 个字符"));
        }
        self.remote.search(query).await
    }

    /// 读取小说详情。
    ///
    /// # 参数
    /// - `novel_id`：正整数小说编号。
    ///
    /// # 错误
    /// 编号非法或上游请求失败时返回错误。
    pub(crate) async fn novel_detail(&self, novel_id: i64) -> AppResult<NovelDetail> {
        validate_novel_id(novel_id).map_err(AppError::invalid_input)?;
        self.remote.novel_detail(novel_id).await
    }

    /// 读取有声专辑对应的作品详情。
    ///
    /// # 参数
    /// - `album_id`：有声专辑编号。
    /// - `novel_id`：所属小说编号。
    ///
    /// # 错误
    /// 任一编号非法或上游请求失败时返回错误。
    pub(crate) async fn audio_detail(
        &self,
        album_id: i64,
        novel_id: i64,
    ) -> AppResult<NovelDetail> {
        validate_novel_id(album_id).map_err(AppError::invalid_input)?;
        validate_novel_id(novel_id).map_err(AppError::invalid_input)?;
        self.remote.audio_detail(album_id, novel_id).await
    }

    /// 读取漫画作品详情。
    ///
    /// # 参数
    /// - `comic_id`：正整数漫画编号。
    ///
    /// # 错误
    /// 编号非法或上游请求失败时返回错误。
    pub(crate) async fn comic_detail(&self, comic_id: i64) -> AppResult<NovelDetail> {
        validate_novel_id(comic_id).map_err(AppError::invalid_input)?;
        self.remote.comic_detail(comic_id).await
    }

    /// 读取文字分卷目录并标记本地已有章节。
    ///
    /// # 参数
    /// - `novel_id`：正整数小说编号。
    ///
    /// # 错误
    /// 编号非法、上游无章节或本地书库读取失败时返回错误。
    pub(crate) async fn chapter_volumes(&self, novel_id: i64) -> AppResult<Vec<ChapterVolume>> {
        validate_novel_id(novel_id).map_err(AppError::invalid_input)?;
        let mut volumes = self.remote.text_volumes(novel_id).await?;
        if volumes.is_empty() {
            return Err(AppError::not_found("该作品没有可用的小说章节"));
        }
        let downloaded = self.store.downloaded_ids("text", None)?;
        for volume in &mut volumes {
            for chapter in &mut volume.chapters {
                chapter.downloaded = downloaded.contains(&chapter.chap_id);
            }
        }
        Ok(volumes)
    }

    /// 读取有声目录；未提供专辑编号时先按标题解析。
    ///
    /// # 参数
    /// - `novel_id`：小说编号。
    /// - `album_id`：可选的有声专辑编号。
    /// - `title_hint`：用于消歧或查找专辑的标题提示。
    ///
    /// # 错误
    /// 编号非法、专辑无法确定、上游请求失败或本地书库读取失败时返回错误。
    pub(crate) async fn audio_catalog(
        &self,
        novel_id: i64,
        album_id: Option<i64>,
        title_hint: Option<String>,
    ) -> AppResult<AudioCatalog> {
        validate_novel_id(novel_id).map_err(AppError::invalid_input)?;
        let album_id = match album_id {
            Some(album_id) => {
                validate_novel_id(album_id).map_err(AppError::invalid_input)?;
                album_id
            }
            None => {
                self.remote
                    .audio_album_id(novel_id, title_hint.as_deref())
                    .await?
            }
        };
        let (title, chapters) = self
            .remote
            .audio_catalog(album_id, title_hint.as_deref())
            .await?;
        let downloaded = self.store.downloaded_ids("audio", Some(&title))?;
        Ok(AudioCatalog {
            title,
            chapters: chapters
                .into_iter()
                .map(|chapter| AudioChapterSummary {
                    downloaded: downloaded.contains(&chapter.id),
                    id: chapter.id,
                    title: chapter.title,
                    volume: chapter.volume,
                    is_vip: chapter.is_vip,
                    is_unlocked: chapter.is_unlocked,
                })
                .collect(),
        })
    }

    /// 读取漫画目录并标记本地已有章节。
    ///
    /// # 参数
    /// - `comic_id`：漫画编号。
    /// - `source_path`：可选的网页来源路径。
    /// - `title_hint`：用于解析来源目录的标题提示。
    ///
    /// # 错误
    /// 编号非法、来源目录无法解析、上游请求失败或本地书库读取失败时返回错误。
    pub(crate) async fn comic_catalog(
        &self,
        comic_id: i64,
        source_path: Option<String>,
        title_hint: Option<String>,
    ) -> AppResult<ComicCatalog> {
        validate_novel_id(comic_id).map_err(AppError::invalid_input)?;
        let (title, folder) = self
            .remote
            .comic_source(comic_id, source_path.as_deref(), title_hint.as_deref())
            .await?;
        let chapters = self.remote.comic_catalog(&folder).await?;
        let downloaded = self.store.downloaded_ids("comic", Some(&title))?;
        Ok(ComicCatalog {
            title,
            chapters: chapters
                .into_iter()
                .map(|chapter| ComicChapterSummary {
                    id: chapter.id,
                    title: chapter.title,
                    is_vip: chapter.is_vip,
                    is_unlocked: chapter.is_unlocked,
                    downloaded: downloaded.contains(&chapter.id),
                })
                .collect(),
        })
    }

    /// 读取当前账号的公开书架并去重。
    ///
    /// # 参数
    /// - `_force_refresh`：保留给调用方的刷新意图；当前每次都会重新请求。
    ///
    /// # 错误
    /// 未登录或上游书架请求失败时返回错误。
    pub(crate) async fn bookshelf(
        &self,
        _force_refresh: Option<bool>,
    ) -> AppResult<BookshelfCollection> {
        let public_shelf = self.remote.bookshelf().await?;
        let categories = public_shelf.categories;
        let mut items = Vec::new();
        for (category, record) in public_shelf.items {
            if items.iter().any(|item: &SearchNovel| {
                item.novel_id == record.id && item.bookshelf_type.as_deref() == Some(record.kind)
            }) {
                continue;
            }
            items.push(SearchNovel {
                novel_id: record.id,
                media_id: None,
                novel_name: record.title,
                author_name: record.author,
                novel_cover: record.cover,
                last_update_time: String::new(),
                bookshelf_name: Some(category),
                bookshelf_type: Some(record.kind.to_string()),
                source_path: record.source_path,
            });
        }
        Ok(BookshelfCollection { categories, items })
    }
}
