//! 下载任务用例。

use crate::application::ports::DownloadGateway;
use crate::domain::download::DownloadJob;
use crate::domain::error::{AppError, AppResult};
use crate::domain::ids::validate_positive_id;

/// 负责校验下载请求并委托给下载适配器的应用服务。
pub(crate) struct DownloadService<'a, G: DownloadGateway> {
    /// 本地下载队列实现。
    gateway: &'a G,
}

impl<'a, G: DownloadGateway> DownloadService<'a, G> {
    /// 创建下载服务。
    ///
    /// # 参数
    /// - `gateway`：下载任务网关。
    pub(crate) fn new(gateway: &'a G) -> Self {
        Self { gateway }
    }

    /// 创建文字小说下载任务。
    ///
    /// # 参数
    /// - `novel_id`：小说编号。
    /// - `title`：用于任务展示和本地目录的名称。
    /// - `chapter_ids`：待下载章节编号。
    ///
    /// # 错误
    /// 编号、标题或章节选择非法，或网关拒绝创建任务时返回错误。
    pub(crate) async fn create_text(
        &self,
        novel_id: i64,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<DownloadJob> {
        validate_download(
            &title,
            &chapter_ids,
            "小说标题无效",
            "章节选择无效",
            novel_id,
            "小说",
        )?;
        self.gateway.create_text(novel_id, title, chapter_ids).await
    }

    /// 创建有声下载任务。
    ///
    /// # 参数
    /// - `novel_id`：小说编号。
    /// - `album_id`：可选的有声专辑编号；提供时必须为正数。
    /// - `title`：任务展示和本地目录名称。
    /// - `chapter_ids`：待下载章节编号。
    ///
    /// # 错误
    /// 编号、标题或章节选择非法，或网关拒绝创建任务时返回错误。
    pub(crate) async fn create_audio(
        &self,
        novel_id: i64,
        album_id: Option<i64>,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<DownloadJob> {
        validate_download(
            &title,
            &chapter_ids,
            "小说标题无效",
            "章节选择无效",
            novel_id,
            "小说",
        )?;
        if let Some(album_id) = album_id {
            validate_positive_id(album_id, "有声专辑").map_err(AppError::invalid_input)?;
        }
        self.gateway
            .create_audio(novel_id, album_id, title, chapter_ids)
            .await
    }

    /// 创建漫画下载任务。
    ///
    /// # 参数
    /// - `comic_id`：漫画编号。
    /// - `source_path`：可选的网页来源路径。
    /// - `title`：任务展示和本地目录名称。
    /// - `chapter_ids`：待下载章节编号。
    ///
    /// # 错误
    /// 编号、标题或章节选择非法，或网关拒绝创建任务时返回错误。
    pub(crate) async fn create_comic(
        &self,
        comic_id: i64,
        source_path: Option<String>,
        title: String,
        chapter_ids: Vec<i64>,
    ) -> AppResult<DownloadJob> {
        validate_download(
            &title,
            &chapter_ids,
            "漫画标题无效",
            "漫画章节选择无效",
            comic_id,
            "漫画",
        )?;
        self.gateway
            .create_comic(comic_id, source_path, title, chapter_ids)
            .await
    }

    /// 返回当前全部下载任务。
    ///
    /// # 错误
    /// 下载队列状态不可用时返回错误。
    pub(crate) fn list(&self) -> AppResult<Vec<DownloadJob>> {
        self.gateway.list()
    }

    /// 暂停指定下载任务。
    ///
    /// # 参数
    /// - `job_id`：任务唯一标识。
    ///
    /// # 错误
    /// 任务不存在或当前状态无法暂停时返回错误。
    pub(crate) fn pause(&self, job_id: &str) -> AppResult<DownloadJob> {
        self.gateway.pause(job_id)
    }

    /// 恢复指定下载任务。
    ///
    /// # 参数
    /// - `job_id`：任务唯一标识。
    ///
    /// # 错误
    /// 任务不存在、缺少恢复参数或当前状态无法恢复时返回错误。
    pub(crate) fn resume(&self, job_id: &str) -> AppResult<DownloadJob> {
        self.gateway.resume(job_id)
    }

    /// 删除指定下载任务及其队列状态。
    ///
    /// # 参数
    /// - `job_id`：任务唯一标识。
    ///
    /// # 错误
    /// 任务不存在或队列状态更新失败时返回错误。
    pub(crate) fn delete(&self, job_id: &str) -> AppResult<()> {
        self.gateway.delete(job_id)
    }
}

/// 校验所有下载任务共有的标题、编号和章节参数。
///
/// # 参数
/// - `title`：下载标题，去除空白后长度必须为 1 到 200 个字符。
/// - `chapter_ids`：非空且全部为正数的章节编号。
/// - `title_error`：标题非法时返回的消息。
/// - `chapter_error`：章节选择非法时返回的消息。
/// - `id`：作品或媒体编号。
/// - `id_label`：编号错误消息中的资源名称。
///
/// # 错误
/// 任一字段不满足上述约束时返回参数错误。
fn validate_download(
    title: &str,
    chapter_ids: &[i64],
    title_error: &str,
    chapter_error: &str,
    id: i64,
    id_label: &str,
) -> AppResult<()> {
    validate_positive_id(id, id_label).map_err(AppError::invalid_input)?;
    if title.trim().is_empty() || title.chars().count() > 200 {
        return Err(AppError::invalid_input(title_error));
    }
    if chapter_ids.is_empty() || chapter_ids.iter().any(|id| *id <= 0) {
        return Err(AppError::invalid_input(chapter_error));
    }
    Ok(())
}
