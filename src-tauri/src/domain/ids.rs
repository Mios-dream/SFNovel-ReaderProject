//! 公共标识校验。
//!
//! 上游编号在进入 URL 之前必须为正数。这里按资源类型提供校验函数，让错误消息
//! 说的是调用方真正传入的东西（小说、章节、专辑还是漫画）。
//!
//! 小说编号在跨层传递时使用 [`NovelId`]：类型存在即代表"已校验"，内层不再重复校验，
//! 也不会与专辑编号、漫画编号这些同为 `i64` 的编号混用。

use crate::domain::error::{AppError, AppResult};

/// 已通过校验的小说编号。
///
/// 构造是唯一的校验点（[`NovelId::new`] / `TryFrom<i64>`），因此拿到该类型的函数
/// 可以直接使用，无需再调用 [`validate_novel_id`]。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub(crate) struct NovelId(i64);

impl NovelId {
    /// 校验并构造小说编号。
    ///
    /// # 参数
    /// - `value`：上游提供的小说编号。
    ///
    /// # 错误
    /// 编号不为正数时返回参数错误。
    pub(crate) fn new(value: i64) -> AppResult<Self> {
        validate_novel_id(value)?;
        Ok(Self(value))
    }

    /// 返回上游接口使用的基础整数编号。
    pub(crate) fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for NovelId {
    type Error = AppError;

    /// 校验并转换小说编号。
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// 校验上游资源编号。
///
/// `label` 用于说明编号属于哪类资源，例如「小说」「章节」。
pub(crate) fn validate_positive_id(value: i64, label: &str) -> AppResult<()> {
    if value <= 0 {
        return Err(AppError::invalid_input(format!("{label}编号无效")));
    }
    Ok(())
}

/// 校验公开 SF 小说编号。
pub(crate) fn validate_novel_id(novel_id: i64) -> AppResult<()> {
    validate_positive_id(novel_id, "小说")
}

/// 校验有声专辑编号。
pub(crate) fn validate_album_id(album_id: i64) -> AppResult<()> {
    validate_positive_id(album_id, "有声专辑")
}

/// 校验漫画编号。
pub(crate) fn validate_comic_id(comic_id: i64) -> AppResult<()> {
    validate_positive_id(comic_id, "漫画")
}

/// 校验章节编号。
pub(crate) fn validate_chapter_id(chapter_id: i64) -> AppResult<()> {
    validate_positive_id(chapter_id, "章节")
}

/// 校验评论编号。
pub(crate) fn validate_comment_id(comment_id: i64) -> AppResult<()> {
    validate_positive_id(comment_id, "评论")
}
