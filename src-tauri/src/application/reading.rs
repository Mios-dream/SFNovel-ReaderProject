//! 在线阅读用例。

use crate::application::ports::OnlineReader;
use crate::domain::content::{OnlineAudioTrack, OnlineComicChapter, OnlineTextChapter};
use crate::domain::error::{AppError, AppResult};
use crate::domain::ids::validate_novel_id;

/// 对在线文字、漫画和音频读取执行统一参数校验的服务。
pub(crate) struct ReadingService<'a, R: OnlineReader> {
    /// 在线内容读取实现。
    reader: &'a R,
}

impl<'a, R: OnlineReader> ReadingService<'a, R> {
    /// 创建在线阅读服务。
    ///
    /// # 参数
    /// - `reader`：在线内容读取实现。
    pub(crate) fn new(reader: &'a R) -> Self {
        Self { reader }
    }

    /// 读取在线文字章节。
    ///
    /// # 参数
    /// - `novel_id`：小说编号。
    /// - `chapter_id`：章节编号。
    ///
    /// # 错误
    /// 任一编号非法或上游读取失败时返回错误。
    pub(crate) async fn text(
        &self,
        novel_id: i64,
        chapter_id: i64,
    ) -> AppResult<OnlineTextChapter> {
        validate_novel_id(novel_id).map_err(AppError::invalid_input)?;
        validate_novel_id(chapter_id).map_err(AppError::invalid_input)?;
        self.reader.load_text(novel_id, chapter_id).await
    }

    /// 读取在线漫画章节。
    ///
    /// # 参数
    /// - `comic_id`：漫画编号。
    /// - `chapter_id`：章节编号。
    /// - `chapter_title`：章节标题，长度必须为 1 到 300 个字符。
    /// - `source_path`：保留的来源路径。
    ///
    /// # 错误
    /// 编号或标题非法、上游读取失败时返回错误。
    pub(crate) async fn comic(
        &self,
        comic_id: i64,
        chapter_id: i64,
        chapter_title: String,
        source_path: Option<String>,
    ) -> AppResult<OnlineComicChapter> {
        validate_novel_id(comic_id).map_err(AppError::invalid_input)?;
        validate_novel_id(chapter_id).map_err(AppError::invalid_input)?;
        let title = chapter_title.trim();
        if title.is_empty() || title.chars().count() > 300 {
            return Err(AppError::invalid_input("漫画章节标题无效"));
        }
        self.reader
            .load_comic(comic_id, chapter_id, title, source_path.as_deref())
            .await
    }

    /// 读取在线音频流地址。
    ///
    /// # 参数
    /// - `chapter_id`：音频章节编号。
    /// - `chapter_title`：前端缓存目录中的章节标题。
    /// - `chapter_volume`：前端缓存目录中的分卷名称。
    ///
    /// # 错误
    /// 章节编号或标题非法，或上游读取失败时返回错误。
    pub(crate) async fn audio(
        &self,
        chapter_id: i64,
        chapter_title: String,
        chapter_volume: String,
    ) -> AppResult<OnlineAudioTrack> {
        validate_novel_id(chapter_id).map_err(AppError::invalid_input)?;
        let chapter_title = chapter_title.trim();
        if chapter_title.is_empty() || chapter_title.chars().count() > 300 {
            return Err(AppError::invalid_input("有声章节标题无效"));
        }
        self.reader
            .load_audio(chapter_id, chapter_title, chapter_volume.trim())
            .await
    }
}
