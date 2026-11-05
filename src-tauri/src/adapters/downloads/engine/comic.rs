//! 漫画章节下载流程。
//!
//! 与文字、有声下载保持同一形状：准备目录与作品信息、拉取目录筛出章节、
//! 逐章逐页下载、汇总结果。

use super::assets::{enrich_comic_metadata, fetch_asset};
use super::prepare::{
    announce_started, format_skipped, prepare_book, BookSetup, DownloadedField,
};
use super::{finish, JobContext};
use crate::adapters::downloads::registry;
use crate::adapters::downloads::DownloadState;
use crate::adapters::storage::replace_file;
use crate::application::ports::RemoteGateway;
use crate::domain::catalog::ComicChapter;
use crate::domain::download::JobStatus;
use crate::domain::error::{AppError, AppResult};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Manager;

/// 从目录中筛出已选中且可下载的章节，保持目录顺序。
///
/// # 参数
/// - `catalog`：上游返回的完整漫画目录。
/// - `requested`：用户勾选的章节编号集合。
fn select_chapters(catalog: Vec<ComicChapter>, requested: &HashSet<i64>) -> Vec<ComicChapter> {
    catalog
        .into_iter()
        .filter(|chapter| requested.contains(&chapter.id) && chapter.is_unlocked.is_selectable())
        .collect()
}

/// 章节下载完成的标记文件名；存在即代表整章页面齐全。
const COMPLETE_MARKER: &str = ".complete";

/// 根据漫画页面 URL 选择保存扩展名。
///
/// # 参数
/// - `url`：上游页面地址。
fn comic_image_extension(url: &str) -> &'static str {
    let path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    if path.ends_with(".png") {
        "png"
    } else if path.ends_with(".webp") {
        "webp"
    } else if path.ends_with(".jpeg") {
        "jpeg"
    } else {
        "jpg"
    }
}

/// 一次漫画下载任务的运行时状态。
struct ComicDownload<'a> {
    context: JobContext<'a>,
    book: BookSetup,
    comic_id: i64,
    /// 被跳过的章节及原因，任务结束时汇总成提示。
    skipped: Vec<(String, String)>,
}

impl<'a> ComicDownload<'a> {
    /// 阶段一：准备目录、写入作品信息并补全元数据。
    ///
    /// # 错误
    /// 来源解析、清单写入失败时返回错误；风控直接中止。
    async fn prepare(
        context: JobContext<'a>,
        comic_id: i64,
        source_path: Option<&str>,
        title: &str,
    ) -> AppResult<Self> {
        let mut book = prepare_book(context, title).await?;
        let (_, folder) = book
            .source
            .comic_source(comic_id, source_path, Some(title))
            .await?;
        book.write_identity(|manifest| {
            let comic = manifest.comic.get_or_insert_with(Default::default);
            comic.id = Some(comic_id);
            comic.title = Some(title.to_string());
            comic
                .author
                .get_or_insert_with(|| "未知作者".to_string());
            comic
                .description
                .get_or_insert_with(|| "暂无简介".to_string());
            comic.online_path.get_or_insert(folder.clone());
        })?;
        let enriched = {
            let BookSetup {
                source,
                directory,
                manifest,
                ..
            } = &mut book;
            let comic = manifest
                .comic
                .as_mut()
                .ok_or_else(|| AppError::internal("漫画元数据初始化失败"))?;
            enrich_comic_metadata(source, directory, comic, comic_id).await
        };
        book.absorb_enrich_failure(enriched)?;
        book.persist()?;
        announce_started(&context);
        Ok(Self {
            context,
            book,
            comic_id,
            skipped: Vec::new(),
        })
    }

    /// 阶段二：拉取目录，筛出可下载的选中章节。
    ///
    /// # 错误
    /// 目录请求失败或没有任何匹配章节时返回错误。
    async fn plan(&mut self, chapter_ids: &[i64]) -> AppResult<Vec<ComicChapter>> {
        let catalog = self
            .book
            .source
            .comic_catalog(self.comic_id)
            .await?;
        let requested = chapter_ids.iter().copied().collect::<HashSet<_>>();
        let chapters = select_chapters(catalog, &requested);
        if chapters.is_empty() {
            return Err(AppError::invalid_input("所选漫画章节不在当前目录中"));
        }
        Ok(chapters)
    }

    /// 阶段三：逐章逐页下载。
    ///
    /// # 返回
    /// 本轮实际尝试的章节数，供收尾判断是否全部失败。
    ///
    /// # 错误
    /// 任务被暂停、触发风控、非 VIP 章节失败或存储失败时中止；
    /// VIP 章节不可下载时跳过并继续。
    async fn download(&mut self, chapters: Vec<ComicChapter>) -> AppResult<usize> {
        let comic_directory = self.book.media_directory("comic", "漫画")?;
        let mut downloaded = self.book.take_downloaded_ids(DownloadedField::Comic);
        let total = chapters.len();
        for (chapter_index, chapter) in chapters.into_iter().enumerate() {
            if self.context.cancelled.load(Ordering::Relaxed) {
                return Err(AppError::cancelled("下载已暂停"));
            }
            self.announce_reading(chapter_index, total, &chapter.title);
            if !self.save_chapter(&comic_directory, &chapter).await? {
                continue;
            }
            downloaded.insert(chapter.id);
            self.book
                .save_downloaded_ids(DownloadedField::Comic, &downloaded)?;
            self.announce_finished(chapter_index, total, &chapter.title);
            self.wait_between_requests().await;
        }
        self.book
            .save_downloaded_ids(DownloadedField::Comic, &downloaded)?;
        Ok(total)
    }

    /// 下载单章的全部页面，返回是否计入进度。
    ///
    /// 返回 `false` 表示该章被跳过：既不推进进度，也不做节流等待。
    ///
    /// # 错误
    /// 风控、非 VIP 章节失败、目录或文件写入失败时返回错误。
    async fn save_chapter(
        &mut self,
        comic_directory: &Path,
        chapter: &ComicChapter,
    ) -> AppResult<bool> {
        let chapter_directory = comic_directory.join(format!("{:06}", chapter.id));
        let marker = chapter_directory.join(COMPLETE_MARKER);
        if marker.is_file() {
            return Ok(true);
        }
        let images = match self
            .book
            .source
            .comic_images(self.comic_id, &chapter.path_name)
            .await
        {
            Ok(images) => images,
            Err(error) if error.is_risk_control() => return Err(error),
            Err(error) => {
                if !chapter.is_vip {
                    return Err(error);
                }
                self.skip(chapter, error, "漫画章节不可下载，已跳过");
                return Ok(false);
            }
        };
        fs::create_dir_all(&chapter_directory)
            .map_err(|error| AppError::storage(format!("无法创建漫画章节目录：{error}")))?;
        let page_count = images.len();
        for (page_index, image) in images.iter().enumerate() {
            if self.context.cancelled.load(Ordering::Relaxed) {
                return Err(AppError::cancelled("下载已暂停"));
            }
            self.notify(&format!(
                "正在下载：{}（{}/{page_count})",
                chapter.title,
                page_index + 1
            ));
            let extension = comic_image_extension(image);
            let target = chapter_directory.join(format!("{:03}.{extension}", page_index + 1));
            if target.is_file() {
                continue;
            }
            let payload = match fetch_asset(
                &self.book.source,
                image,
                "https://m.sfacg.com/",
                "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
                "漫画图片",
            )
            .await
            {
                Ok(payload) => payload,
                Err(error) if error.is_risk_control() => return Err(error),
                Err(error) => {
                    if !chapter.is_vip {
                        return Err(error);
                    }
                    self.skip(chapter, error, "VIP 漫画章节不可下载，已跳过");
                    return Ok(false);
                }
            };
            replace_file(&target, &payload, "漫画图片")?;
        }
        fs::write(&marker, chapter.title.as_bytes())
            .map_err(|error| AppError::storage(format!("无法完成漫画章节写入：{error}")))?;
        Ok(true)
    }

    /// 记录被跳过的章节并更新提示文案。
    fn skip(&mut self, chapter: &ComicChapter, error: AppError, notice: &str) {
        log_warn!(
            "[sfacg] comic chapter skipped: chapter_id={}, title={}, error={error}",
            chapter.id,
            chapter.title
        );
        self.skipped.push((chapter.title.clone(), error.to_string()));
        self.notify(&format!("{notice}：{}", chapter.title));
    }

    /// 阶段四：汇总结果。
    ///
    /// # 错误
    /// 有章节被跳过时返回汇总提示，否则返回书库里的漫画目录名。
    fn into_result(self) -> AppResult<String> {
        if self.skipped.is_empty() {
            return Ok("comic".to_string());
        }
        Err(AppError::upstream(format_skipped(&self.skipped, "VIP 漫画章节")))
    }

    /// 推送"正在读取"进度。
    fn announce_reading(&self, index: usize, total: usize, chapter_title: &str) {
        let progress = 2 + ((index * 94) / total) as u8;
        self.announce(progress, &format!("正在读取：{chapter_title}"), false);
    }

    /// 推送单章完成进度。
    fn announce_finished(&self, index: usize, total: usize, chapter_title: &str) {
        let progress = 2 + (((index + 1) * 94) / total) as u8;
        self.announce(progress, &format!("已完成：{chapter_title}"), true);
    }

    /// 更新进度与提示，`with_library` 决定是否同时广播书库变更。
    fn announce(&self, progress: u8, message: &str, with_library: bool) {
        let JobContext {
            app,
            state,
            job_id,
            ..
        } = self.context;
        registry::update(state, job_id, |job| {
            job.status = JobStatus::Downloading;
            job.progress = progress;
            job.message = message.to_string();
        });
        registry::emit_job(app, state, job_id);
        if with_library {
            registry::emit_library(app, state, job_id);
        }
    }

    /// 只更新提示文案，不改状态与进度。
    fn notify(&self, message: &str) {
        let JobContext {
            app,
            state,
            job_id,
            ..
        } = self.context;
        registry::update(state, job_id, |job| job.message = message.to_string());
        registry::emit_job(app, state, job_id);
    }

    /// 按设置的最小间隔等待，避免触发上游限流。
    async fn wait_between_requests(&self) {
        tokio::time::sleep(std::time::Duration::from_millis(
            self.book.settings.request_interval_ms,
        ))
        .await;
    }
}

/// 执行漫画下载任务并发布最终状态。
///
/// # 参数
/// - `app`：Tauri 应用句柄。
/// - `job_id`：任务唯一标识。
/// - `comic_id`：漫画编号。
/// - `source_path`：可选的网页来源路径。
/// - `title`：本地目录与展示名称。
/// - `chapter_ids`：待下载章节编号。
/// - `cancelled`：任务取消标记。
pub(super) async fn run(
    app: tauri::AppHandle,
    job_id: String,
    comic_id: i64,
    source_path: Option<String>,
    title: String,
    chapter_ids: Vec<i64>,
    cancelled: Arc<AtomicBool>,
) {
    let state = app.state::<DownloadState>();
    let context = JobContext {
        app: &app,
        state: state.inner(),
        job_id: &job_id,
        cancelled: cancelled.as_ref(),
    };
    let result = run_inner(
        &context,
        comic_id,
        source_path.as_deref(),
        &title,
        &chapter_ids,
    )
    .await;
    finish(&context, result, "漫画章节下载完成", true);
}

/// 依次执行四个阶段。
async fn run_inner(
    context: &JobContext<'_>,
    comic_id: i64,
    source_path: Option<&str>,
    title: &str,
    chapter_ids: &[i64],
) -> AppResult<String> {
    let mut task = ComicDownload::prepare(*context, comic_id, source_path, title).await?;
    let chapters = task.plan(chapter_ids).await?;
    task.download(chapters).await?;
    task.into_result()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::catalog::ComicUnlockState;

    /// 构造一条漫画目录章节。
    fn chapter(id: i64, unlock: ComicUnlockState) -> ComicChapter {
        ComicChapter {
            id,
            title: format!("第 {id} 话"),
            path_name: format!("c{id}"),
            is_vip: true,
            is_unlocked: unlock,
        }
    }

    #[test]
    fn select_chapters_drops_locked_and_unselected() {
        let catalog = vec![
            chapter(1, ComicUnlockState::Unlocked),
            chapter(2, ComicUnlockState::Locked),
            chapter(3, ComicUnlockState::Unknown),
        ];
        let requested = [1, 2, 3].into_iter().collect::<HashSet<_>>();

        let selected = select_chapters(catalog, &requested);

        assert_eq!(
            selected.iter().map(|chapter| chapter.id).collect::<Vec<_>>(),
            vec![1, 3],
            "已锁定章节应被排除，状态未知的章节允许尝试"
        );
    }

    #[test]
    fn select_chapters_keeps_only_requested() {
        let catalog = vec![
            chapter(1, ComicUnlockState::Unlocked),
            chapter(2, ComicUnlockState::Unlocked),
        ];

        let selected = select_chapters(catalog, &[2].into_iter().collect::<HashSet<_>>());

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, 2);
    }

    #[test]
    fn select_chapters_returns_empty_for_empty_catalog() {
        assert!(select_chapters(Vec::new(), &[1].into_iter().collect::<HashSet<_>>()).is_empty());
    }
}
